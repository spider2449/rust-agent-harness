//! Private, closed v1 persistence for inactive Desktop model preferences.

use crate::{
    DesktopModelProvider, DesktopModelSelection, ProviderEndpoint, ProviderHost, ProviderScheme,
};
use serde::Deserialize;
use serde::de::{self, MapAccess, Visitor};
use std::{
    fmt, fs, io,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(test)]
use std::sync::{Mutex, OnceLock};

const FILE: &str = "desktop-preferences.json";
const MAX_BYTES: usize = 4096;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

// This seam is deliberately test-only: production always uses the native file APIs.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TestFault {
    Create,
    Write,
    Sync,
    FirstMove,
    ReplaceAccessDeniedFallbackSucceeds,
    ReplaceAccessDeniedFallbackFails,
    ReplaceOther,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestOperation {
    Create,
    Write,
    Sync,
    FirstMove,
    Replace,
    FallbackMove,
    Cleanup,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TestWriteAccounting {
    pub(crate) destination_write_attempts: u64,
    pub(crate) temp_creations: u64,
    pub(crate) native_replacements_or_moves: u64,
}

#[cfg(test)]
#[derive(Default)]
struct TestState {
    fault: Option<(PathBuf, TestFault)>,
    operations: Vec<(PathBuf, TestOperation)>,
    writes: Vec<PathBuf>,
}
#[cfg(test)]
static TEST_STATE: OnceLock<Mutex<TestState>> = OnceLock::new();
#[cfg(test)]
static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
fn test_state() -> &'static Mutex<TestState> {
    TEST_STATE.get_or_init(|| Mutex::new(TestState::default()))
}
#[cfg(test)]
pub(crate) fn test_lock() -> &'static Mutex<()> {
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}
#[cfg(test)]
fn test_fault(path: &Path) -> Option<TestFault> {
    test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .fault
        .as_ref()
        .and_then(|(p, f)| (p == path).then_some(*f))
}
#[cfg(test)]
fn test_operation(path: &Path, operation: TestOperation) {
    test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .operations
        .push((path.to_owned(), operation));
}
#[cfg(test)]
pub(crate) fn set_test_fault(path: PathBuf, fault: TestFault) {
    let mut state = test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.fault = Some((path, fault));
    state.operations.clear();
}
#[cfg(test)]
pub(crate) fn clear_test_state() {
    *test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = TestState::default();
}
#[cfg(test)]
pub(crate) fn reset_test_accounting() {
    let mut state = test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.operations.clear();
    state.writes.clear();
}
#[cfg(test)]
pub(crate) fn test_write_accounting(path: &Path) -> TestWriteAccounting {
    let state = test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let operation_count = |operation| {
        state
            .operations
            .iter()
            .filter(|(candidate, value)| candidate == path && *value == operation)
            .count() as u64
    };
    TestWriteAccounting {
        destination_write_attempts: state.writes.iter().filter(|p| p.as_path() == path).count()
            as u64,
        temp_creations: operation_count(TestOperation::Create),
        native_replacements_or_moves: operation_count(TestOperation::FirstMove)
            + operation_count(TestOperation::Replace)
            + operation_count(TestOperation::FallbackMove),
    }
}
#[cfg(test)]
fn test_operations(path: &Path) -> Vec<TestOperation> {
    test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .operations
        .iter()
        .filter_map(|(p, op)| (p == path).then_some(*op))
        .collect()
}
#[cfg(test)]
fn test_write_attempts(path: &Path) -> u64 {
    test_state()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .writes
        .iter()
        .filter(|p| p.as_path() == path)
        .count() as u64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Warning {
    RestoreFailed,
    SaveFailed,
}

#[derive(Debug)]
pub(crate) struct Preferences {
    directory: PathBuf,
    warning: Option<Warning>,
}

impl Preferences {
    pub(crate) fn start(directory: PathBuf) -> (Self, DesktopModelSelection) {
        cleanup_temps(&directory);
        let path = directory.join(FILE);
        let loaded = match bounded_read(&path) {
            Ok(Some(bytes)) => match parse(&bytes) {
                Ok(selection) => (None, selection),
                Err(()) => (
                    Some(Warning::RestoreFailed),
                    DesktopModelSelection::default(),
                ),
            },
            Ok(None) => (None, DesktopModelSelection::default()),
            Err(()) => (
                Some(Warning::RestoreFailed),
                DesktopModelSelection::default(),
            ),
        };
        (
            Self {
                directory,
                warning: loaded.0,
            },
            loaded.1,
        )
    }

    pub(crate) fn take_warning(&mut self) -> Option<Warning> {
        self.warning.take()
    }

    pub(crate) fn save(&mut self, selection: &DesktopModelSelection) -> Result<(), Warning> {
        let bytes = canonical(selection).map_err(|_| Warning::SaveFailed)?;
        let destination = self.directory.join(FILE);
        if matches!(bounded_read(&destination), Ok(Some(existing)) if existing == bytes) {
            return Ok(());
        }
        atomic(&self.directory, &bytes).map_err(|_| Warning::SaveFailed)
    }

    pub(crate) fn reset(&mut self) -> Result<(), Warning> {
        self.save(&DesktopModelSelection::default())
    }
}

fn bounded_read(path: &Path) -> Result<Option<Vec<u8>>, ()> {
    let metadata = match fs::metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(()),
    };
    if !metadata.is_file() || metadata.len() as usize > MAX_BYTES {
        return Err(());
    }
    let bytes = fs::read(path).map_err(|_| ())?;
    if bytes.is_empty() || bytes.len() > MAX_BYTES || bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(());
    }
    Ok(Some(bytes))
}

fn canonical(selection: &DesktopModelSelection) -> Result<Vec<u8>, ()> {
    selection.validate().map_err(|_| ())?;
    let json = match selection.provider {
        DesktopModelProvider::Inherit => {
            r#"{"version":1,"model":{"provider":"inherit"}}"#.to_owned()
        }
        DesktopModelProvider::OpenAi
        | DesktopModelProvider::Ollama
        | DesktopModelProvider::LmStudio => {
            let provider = provider_name(selection.provider);
            format!(
                r#"{{"version":1,"model":{{"provider":"{provider}","model":{}}}}}"#,
                serde_json::to_string(selection.model.as_ref().ok_or(())?).map_err(|_| ())?
            )
        }
        DesktopModelProvider::LlamaCpp => {
            let endpoint = selection.llama_cpp_endpoint.as_ref().ok_or(())?;
            let ProviderHost::Ip(ip) = &endpoint.host else {
                return Err(());
            };
            if !ip.is_loopback() || endpoint.scheme != ProviderScheme::Http {
                return Err(());
            }
            format!(
                r#"{{"version":1,"model":{{"provider":"llama_cpp","model":{},"endpoint":{{"scheme":"http","host":{},"port":{}}}}}}}"#,
                serde_json::to_string(selection.model.as_ref().ok_or(())?).map_err(|_| ())?,
                serde_json::to_string(&ip.to_string()).map_err(|_| ())?,
                endpoint.port
            )
        }
    };
    let mut bytes = json.into_bytes();
    bytes.push(b'\n');
    if bytes.len() > MAX_BYTES {
        Err(())
    } else {
        Ok(bytes)
    }
}
#[cfg(test)]
pub(crate) fn test_canonical(selection: &DesktopModelSelection) -> Result<Vec<u8>, ()> {
    canonical(selection)
}

fn provider_name(provider: DesktopModelProvider) -> &'static str {
    match provider {
        DesktopModelProvider::OpenAi => "openai",
        DesktopModelProvider::Ollama => "ollama",
        DesktopModelProvider::LmStudio => "lm_studio",
        _ => unreachable!(),
    }
}

#[derive(Default)]
struct Root {
    version: Option<u64>,
    model: Option<Model>,
}
#[derive(Default)]
struct Model {
    provider: Option<String>,
    model: Option<String>,
    endpoint: Option<Endpoint>,
}
#[derive(Default)]
struct Endpoint {
    scheme: Option<String>,
    host: Option<String>,
    port: Option<u16>,
}

fn parse(bytes: &[u8]) -> Result<DesktopModelSelection, ()> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES || bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    let root: Root = serde_json::from_str(text).map_err(|_| ())?;
    if root.version != Some(1) {
        return Err(());
    }
    let model = root.model.ok_or(())?;
    let provider = match model.provider.as_deref() {
        Some("inherit") => DesktopModelProvider::Inherit,
        Some("openai") => DesktopModelProvider::OpenAi,
        Some("ollama") => DesktopModelProvider::Ollama,
        Some("lm_studio") => DesktopModelProvider::LmStudio,
        Some("llama_cpp") => DesktopModelProvider::LlamaCpp,
        _ => return Err(()),
    };
    let endpoint = match model.endpoint {
        Some(endpoint) => {
            if provider != DesktopModelProvider::LlamaCpp
                || endpoint.scheme.as_deref() != Some("http")
            {
                return Err(());
            }
            let host = endpoint.host.ok_or(())?.parse::<IpAddr>().map_err(|_| ())?;
            if !host.is_loopback() || endpoint.port.unwrap_or(0) == 0 {
                return Err(());
            }
            Some(ProviderEndpoint {
                scheme: ProviderScheme::Http,
                host: ProviderHost::Ip(host),
                port: endpoint.port.unwrap(),
            })
        }
        None => None,
    };
    let result = DesktopModelSelection {
        provider,
        model: model.model,
        llama_cpp_endpoint: endpoint,
    };
    result.validate().map_err(|_| ())?;
    Ok(result)
}

macro_rules! closed_map { ($name:ident, $type:ty, { $($field:literal => $slot:ident : $value:ty),* $(,)? }) => {
impl<'de> Deserialize<'de> for $type { fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> { struct V; impl<'de> Visitor<'de> for V { type Value = $type; fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("closed preferences object") } fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<$type, A::Error> { let mut value: $type = Default::default(); while let Some(key) = map.next_key::<String>()? { match key.as_str() { $($field => { if value.$slot.is_some() { return Err(de::Error::duplicate_field($field)); } value.$slot = Some(map.next_value::<$value>()?); }),* _ => return Err(de::Error::unknown_field(&key, &[$($field),*])), } } Ok(value) } } d.deserialize_map(V) } }
}; }
closed_map!(RootMap, Root, { "version" => version: u64, "model" => model: Model });
closed_map!(ModelMap, Model, { "provider" => provider: String, "model" => model: String, "endpoint" => endpoint: Endpoint });
closed_map!(EndpointMap, Endpoint, { "scheme" => scheme: String, "host" => host: String, "port" => port: u16 });

fn temp_name(n: u64) -> String {
    format!("{FILE}.preferences-tmp-{}-{n}", std::process::id())
}
fn is_temp(name: &str) -> bool {
    let Some(value) = name.strip_prefix(&format!("{FILE}.preferences-tmp-")) else {
        return false;
    };
    let mut parts = value.split('-');
    matches!((parts.next(),parts.next(),parts.next()), (Some(a), Some(b), None) if a.parse::<u32>().is_ok() && b.parse::<u64>().is_ok())
}
fn cleanup_temps(directory: &Path) {
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            if is_temp(&entry.file_name().to_string_lossy()) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}
fn atomic(directory: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let target = directory.join(FILE);
    let temporary = directory.join(temp_name(TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)));
    #[cfg(test)]
    {
        let mut state = test_state()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.writes.push(target.clone());
        state
            .operations
            .push((target.clone(), TestOperation::Create));
    }
    #[cfg(test)]
    if test_fault(&target) == Some(TestFault::Create) {
        return Err(io::Error::other("test create failure"));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    use io::Write;
    #[cfg(test)]
    {
        test_operation(&target, TestOperation::Write);
    }
    #[cfg(test)]
    if test_fault(&target) == Some(TestFault::Write) {
        return Err(io::Error::other("test write failure"));
    }
    file.write_all(bytes)?;
    #[cfg(test)]
    {
        test_operation(&target, TestOperation::Sync);
    }
    #[cfg(test)]
    if test_fault(&target) == Some(TestFault::Sync) {
        return Err(io::Error::other("test sync failure"));
    }
    file.sync_all()?;
    drop(file);
    let result = if target.exists() {
        replace(&target, &temporary)
    } else {
        move_file(&temporary, &target)
    };
    if result.is_err() {
        #[cfg(test)]
        {
            test_operation(&target, TestOperation::Cleanup);
        }
        let _ = fs::remove_file(temporary);
    }
    result
}
#[cfg(target_os = "windows")]
fn wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
#[cfg(target_os = "windows")]
fn replace(destination: &Path, temporary: &Path) -> io::Result<()> {
    #[cfg(test)]
    {
        test_operation(destination, TestOperation::Replace);
        match test_fault(destination) {
            Some(TestFault::ReplaceAccessDeniedFallbackSucceeds) => {
                test_operation(destination, TestOperation::FallbackMove);
                fs::rename(temporary, destination)?;
                return Ok(());
            }
            Some(TestFault::ReplaceAccessDeniedFallbackFails) => {
                test_operation(destination, TestOperation::FallbackMove);
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "test fallback failure",
                ));
            }
            Some(TestFault::ReplaceOther) => return Err(io::Error::other("test replace failure")),
            _ => {}
        }
    }
    let d = wide(destination);
    let t = wide(temporary);
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            d.as_ptr(),
            t.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    } != 0
    {
        Ok(())
    } else {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED as i32)
        {
            return Err(error);
        }
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                t.as_ptr(),
                d.as_ptr(),
                windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
                    | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
            )
        } != 0
        {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}
#[cfg(target_os = "windows")]
fn move_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(test)]
    {
        test_operation(destination, TestOperation::FirstMove);
        if test_fault(destination) == Some(TestFault::FirstMove) {
            return Err(io::Error::other("test first move failure"));
        }
    }
    let t = wide(temporary);
    let d = wide(destination);
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            t.as_ptr(),
            d.as_ptr(),
            windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
        )
    } != 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEST_DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time follows Unix epoch")
                .as_nanos();
            let sequence = TEST_DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("rah-desktop-preferences-{timestamp}-{sequence}"));
            fs::create_dir(&path).expect("test preferences directory is created");
            Self(path)
        }

        fn preference_path(&self) -> PathBuf {
            self.0.join(FILE)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn explicit(provider: DesktopModelProvider) -> DesktopModelSelection {
        DesktopModelSelection {
            provider,
            model: Some("model".into()),
            llama_cpp_endpoint: None,
        }
    }

    #[test]
    fn canonical_provider_shapes_round_trip() {
        let mut all = vec![
            (
                DesktopModelSelection::default(),
                r#"{"version":1,"model":{"provider":"inherit"}}"#,
            ),
            (
                explicit(DesktopModelProvider::OpenAi),
                r#"{"version":1,"model":{"provider":"openai","model":"model"}}\n"#,
            ),
            (
                explicit(DesktopModelProvider::Ollama),
                r#"{"version":1,"model":{"provider":"ollama","model":"model"}}\n"#,
            ),
            (
                explicit(DesktopModelProvider::LmStudio),
                r#"{"version":1,"model":{"provider":"lm_studio","model":"model"}}\n"#,
            ),
        ];
        for (host, expected) in [
            (
                "127.0.0.1",
                r#"{"version":1,"model":{"provider":"llama_cpp","model":"model","endpoint":{"scheme":"http","host":"127.0.0.1","port":8080}}}\n"#,
            ),
            (
                "::1",
                r#"{"version":1,"model":{"provider":"llama_cpp","model":"model","endpoint":{"scheme":"http","host":"::1","port":8080}}}\n"#,
            ),
        ] {
            all.push((
                DesktopModelSelection {
                    provider: DesktopModelProvider::LlamaCpp,
                    model: Some("model".into()),
                    llama_cpp_endpoint: Some(ProviderEndpoint {
                        scheme: ProviderScheme::Http,
                        host: ProviderHost::Ip(host.parse().unwrap()),
                        port: 8080,
                    }),
                },
                expected,
            ));
        }
        for (selection, expected) in all {
            let bytes = canonical(&selection).unwrap();
            assert_eq!(
                bytes,
                format!("{}\n", expected.trim_end_matches(r#"\n"#)).as_bytes()
            );
            assert_eq!(parse(&bytes).unwrap(), selection);
            assert!(bytes.ends_with(b"\n"));
            assert!(!std::str::from_utf8(&bytes).unwrap().contains("null"));
            assert!(!std::str::from_utf8(&bytes).unwrap().contains("/v1"));
        }
    }

    #[test]
    fn strict_parser_rejects_duplicates_and_non_loopback() {
        for bytes in [br#"{"version":1,"version":1,"model":{"provider":"inherit"}}"#.as_slice(), br#"{"version":1,"model":{"provider":"inherit","provider":"inherit"}}"#.as_slice(), br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"127.0.0.1","host":"127.0.0.1","port":1}}}"#.as_slice(), br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"192.168.1.4","port":1}}}"#.as_slice()] { assert!(parse(bytes).is_err()); }
    }

    #[test]
    fn strict_parser_rejects_root_model_and_endpoint_matrix() {
        let oversized = format!(
            "{{\"version\":1,\"model\":{{\"provider\":\"inherit\"}}}}{}",
            " ".repeat(MAX_BYTES)
        );
        let invalid = [
            b"".as_slice(), b" \n\t".as_slice(), b"{".as_slice(), b"{} trailing".as_slice(),
            b"\xef\xbb\xbf{\"version\":1,\"model\":{\"provider\":\"inherit\"}}".as_slice(),
            &[0xff][..], oversized.as_bytes(),
            br#"{"model":{"provider":"inherit"}}"#, br#"{"version":2,"model":{"provider":"inherit"}}"#,
            br#"{"version":1}"#, br#"{"version":1,"model":{"provider":"inherit"},"extra":true}"#,
            br#"{"version":1,"model":{"provider":"inherit","extra":true}}"#,
            br#"{"version":1,"model":{"provider":null}}"#, br#"{"version":1,"model":{"provider":"openai","model":null}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":null}}"#,
            br#"{"version":1,"model":{"provider":"invalid","model":"x"}}"#,
            br#"{"version":1,"model":{"provider":"inherit","model":"x"}}"#,
            br#"{"version":1,"model":{"provider":"inherit","endpoint":{"scheme":"http","host":"127.0.0.1","port":1}}}"#,
            br#"{"version":1,"model":{"provider":"openai","model":"x","endpoint":{"scheme":"http","host":"127.0.0.1","port":1}}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x"}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"https","host":"127.0.0.1","port":1}}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"127.0.0.1","port":0}}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"localhost","port":1}}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"10.0.0.1","port":1}}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"fe80::1","port":1}}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"2001:db8::1","port":1}}}"#,
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"127.0.0.1","port":1,"extra":true}}}"#,
        ];
        for bytes in invalid {
            assert!(parse(bytes).is_err(), "{bytes:?}");
        }
        for duplicate in [
            br#"{"version":1,"model":{"provider":"inherit","model":"x","model":"x"}}"#.as_slice(),
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","scheme":"http","host":"127.0.0.1","port":1}}}"#.as_slice(),
            br#"{"version":1,"model":{"provider":"llama_cpp","model":"x","endpoint":{"scheme":"http","host":"127.0.0.1","port":1,"port":1}}}"#.as_slice(),
        ] { assert!(parse(duplicate).is_err(), "{duplicate:?}"); }
    }

    #[test]
    fn strict_parser_accepts_only_literal_loopback_ip_addresses() {
        for host in ["127.0.0.1", "127.255.255.255", "::1"] {
            let source = format!(
                r#"{{"version":1,"model":{{"provider":"llama_cpp","model":"x","endpoint":{{"scheme":"http","host":"{host}","port":1}}}}}}"#
            );
            assert!(parse(source.as_bytes()).is_ok(), "{host}");
        }
    }

    #[test]
    fn model_identifier_is_closed() {
        assert!(crate::validate_model_identifier("x").is_ok());
        assert!(crate::validate_model_identifier(&"x".repeat(256)).is_ok());
        assert!(crate::validate_model_identifier(&("界".repeat(85) + "x")).is_ok());
        assert!(crate::validate_model_identifier(&(String::from("界").repeat(85) + "xx")).is_err());
        assert!(crate::validate_model_identifier("middle space").is_ok());
        for value in [
            "",
            &"x".repeat(257),
            " x",
            "x ",
            "\u{2003}x",
            "x\u{2003}",
            "x\n",
            "x\u{0080}",
            "x\0",
        ] {
            assert!(crate::validate_model_identifier(value).is_err());
        }
    }

    #[test]
    fn restore_invalid_file_is_non_destructive_and_defaults_inactive() {
        let directory = TestDirectory::new();
        let invalid = b"{malformed";
        fs::write(directory.preference_path(), invalid).unwrap();
        let (mut preferences, selection) = Preferences::start(directory.0.clone());
        assert_eq!(selection, DesktopModelSelection::default());
        assert_eq!(preferences.take_warning(), Some(Warning::RestoreFailed));
        assert_eq!(fs::read(directory.preference_path()).unwrap(), invalid);
    }

    #[test]
    fn preferences_only_write_for_persistable_selections_and_reset() {
        let directory = TestDirectory::new();
        let mut preferences = Preferences::start(directory.0.clone()).0;
        let selection = explicit(DesktopModelProvider::OpenAi);
        preferences.save(&selection).unwrap();
        assert_eq!(
            fs::read(directory.preference_path()).unwrap(),
            canonical(&selection).unwrap()
        );
        let retained = fs::read(directory.preference_path()).unwrap();
        let non_loopback = DesktopModelSelection {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("model".into()),
            llama_cpp_endpoint: Some(ProviderEndpoint {
                scheme: ProviderScheme::Http,
                host: ProviderHost::Ip("192.168.1.1".parse().unwrap()),
                port: 8080,
            }),
        };
        assert_eq!(preferences.save(&non_loopback), Err(Warning::SaveFailed));
        assert_eq!(fs::read(directory.preference_path()).unwrap(), retained);
        preferences.reset().unwrap();
        assert_eq!(
            fs::read(directory.preference_path()).unwrap(),
            canonical(&DesktopModelSelection::default()).unwrap()
        );
    }

    #[test]
    fn cleanup_removes_only_owned_preference_temps() {
        let directory = TestDirectory::new();
        let owned = directory.0.join(temp_name(7));
        let keep = [
            directory.preference_path(),
            directory
                .0
                .join("conversation-transcript.json.preferences-tmp-1-1"),
            directory
                .0
                .join("desktop-preferences.json.preferences-tmp-1-extra"),
            directory.0.join("other"),
        ];
        fs::write(&owned, "stale").unwrap();
        for path in &keep {
            fs::write(path, "keep").unwrap();
        }
        let _ = Preferences::start(directory.0.clone());
        assert!(!owned.exists());
        for path in &keep {
            assert!(path.exists(), "{}", path.display());
        }
    }

    #[test]
    fn atomic_failure_matrix_never_damages_existing_destination_or_retries() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = TestDirectory::new();
        let mut preferences = Preferences::start(directory.0.clone()).0;
        let prior = explicit(DesktopModelProvider::OpenAi);
        let next = explicit(DesktopModelProvider::Ollama);
        preferences.save(&prior).unwrap();
        let durable = fs::read(directory.preference_path()).unwrap();
        for fault in [
            TestFault::Create,
            TestFault::Write,
            TestFault::Sync,
            TestFault::ReplaceAccessDeniedFallbackFails,
            TestFault::ReplaceOther,
        ] {
            clear_test_state();
            set_test_fault(directory.preference_path(), fault);
            assert_eq!(
                preferences.save(&next),
                Err(Warning::SaveFailed),
                "{fault:?}"
            );
            assert_eq!(
                fs::read(directory.preference_path()).unwrap(),
                durable,
                "{fault:?}"
            );
            assert_eq!(
                test_write_attempts(&directory.preference_path()),
                1,
                "{fault:?}"
            );
            assert!(
                !test_operations(&directory.preference_path()).contains(&TestOperation::FirstMove),
                "{fault:?}"
            );
        }
        clear_test_state();
    }

    #[test]
    fn first_create_failure_leaves_no_destination_and_uses_only_first_move() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = TestDirectory::new();
        let mut preferences = Preferences::start(directory.0.clone()).0;
        set_test_fault(directory.preference_path(), TestFault::FirstMove);
        assert_eq!(
            preferences.save(&explicit(DesktopModelProvider::OpenAi)),
            Err(Warning::SaveFailed)
        );
        assert!(!directory.preference_path().exists());
        assert_eq!(
            test_operations(&directory.preference_path()),
            vec![
                TestOperation::Create,
                TestOperation::Write,
                TestOperation::Sync,
                TestOperation::FirstMove,
                TestOperation::Cleanup
            ]
        );
        clear_test_state();
    }

    #[test]
    fn replacement_uses_only_documented_access_denied_fallback() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = TestDirectory::new();
        let mut preferences = Preferences::start(directory.0.clone()).0;
        let prior = explicit(DesktopModelProvider::OpenAi);
        let next = explicit(DesktopModelProvider::Ollama);
        preferences.save(&prior).unwrap();
        clear_test_state();
        set_test_fault(
            directory.preference_path(),
            TestFault::ReplaceAccessDeniedFallbackSucceeds,
        );
        preferences.save(&next).unwrap();
        assert_eq!(
            fs::read(directory.preference_path()).unwrap(),
            canonical(&next).unwrap()
        );
        assert_eq!(
            test_operations(&directory.preference_path()),
            vec![
                TestOperation::Create,
                TestOperation::Write,
                TestOperation::Sync,
                TestOperation::Replace,
                TestOperation::FallbackMove
            ]
        );

        clear_test_state();
        set_test_fault(directory.preference_path(), TestFault::ReplaceOther);
        assert_eq!(preferences.save(&prior), Err(Warning::SaveFailed));
        assert_eq!(
            test_operations(&directory.preference_path()),
            vec![
                TestOperation::Create,
                TestOperation::Write,
                TestOperation::Sync,
                TestOperation::Replace,
                TestOperation::Cleanup
            ]
        );
        clear_test_state();
    }

    #[test]
    fn non_persistable_selection_never_attempts_a_filesystem_write() {
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let directory = TestDirectory::new();
        let mut preferences = Preferences::start(directory.0.clone()).0;
        let durable = explicit(DesktopModelProvider::OpenAi);
        preferences.save(&durable).unwrap();
        let prior = fs::read(directory.preference_path()).unwrap();
        clear_test_state();
        let non_loopback = DesktopModelSelection {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("model".into()),
            llama_cpp_endpoint: Some(ProviderEndpoint {
                scheme: ProviderScheme::Http,
                host: ProviderHost::Ip("192.168.1.1".parse().unwrap()),
                port: 8080,
            }),
        };
        assert_eq!(preferences.save(&non_loopback), Err(Warning::SaveFailed));
        assert_eq!(test_write_attempts(&directory.preference_path()), 0);
        assert!(test_operations(&directory.preference_path()).is_empty());
        assert_eq!(fs::read(directory.preference_path()).unwrap(), prior);
    }
}
