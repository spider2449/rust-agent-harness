//! Only compiled in explicit certification test builds; never a Tauri command.
use super::*;
use rah_runtime::experimental::{ConfiguredRuntimeFactory, ModelSelection};
use rah_runtime_codex::certification_support::{
    CertificationVerificationError, ExactCodexCandidate, verify_and_construct_candidate,
};

const PATH: &str = "C:/Users/morefunfun/AppData/Roaming/npm/node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe";
const HASH: &str = "fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d";

fn descriptor() -> ExactCodexCandidate {
    ExactCodexCandidate {
        path: PATH.into(),
        expected_version: "0.160.0".into(),
        expected_sha256: HASH.into(),
    }
}

#[test]
fn certification_frontend_mapping_preserves_private_sources() {
    use std::error::Error;
    for cause in [
        CertificationVerificationError::MissingArtifact { path: PATH.into() },
        CertificationVerificationError::HashMismatch {
            expected: "0".repeat(64),
            actual: HASH.into(),
        },
        CertificationVerificationError::VersionMismatch {
            expected: "0.157.1".into(),
            actual: "codex-cli 0.160.0".into(),
        },
    ] {
        let adapter = rah_runtime_codex::CodexAdapterError::from(cause);
        let frontend = codex_composition::frontend_error(&adapter);
        assert_eq!(frontend, FrontendError::CodexConnectionFailed);
        assert_eq!(
            serde_json::to_string(&frontend).unwrap(),
            "\"codex_connection_failed\""
        );
        let source = adapter
            .source()
            .unwrap()
            .downcast_ref::<CertificationVerificationError>()
            .unwrap();
        match source {
            CertificationVerificationError::MissingArtifact { path } => {
                assert_eq!(path, &std::path::PathBuf::from(PATH))
            }
            CertificationVerificationError::HashMismatch { expected, actual } => {
                assert_eq!(expected, &"0".repeat(64));
                assert_eq!(actual, HASH);
            }
            CertificationVerificationError::VersionMismatch { expected, actual } => {
                assert_eq!(expected, "0.157.1");
                assert_eq!(actual, "codex-cli 0.160.0");
            }
        }
    }
}

fn verification_cause(error: &rah_runtime::RuntimeFailure) -> &CertificationVerificationError {
    use std::error::Error;
    let public = format!(
        "{} {}",
        error,
        serde_json::to_string(error.diagnostic()).unwrap()
    );
    assert!(!public.contains(PATH));
    assert!(!public.contains(HASH));
    assert!(!public.contains(&"0".repeat(64)));
    assert!(!public.contains("0.157.1"));
    assert!(!public.contains("0.160.0"));
    error
        .source()
        .unwrap()
        .downcast_ref::<rah_runtime_codex::CodexAdapterError>()
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<CertificationVerificationError>()
        .unwrap()
}

#[tokio::test]
#[ignore = "explicit exact installed artifact, no inference; Task 510B2 only"]
async fn task510b2_exact_candidate_desktop_smoke() {
    let root =
        std::env::temp_dir().join(format!("rah-task510b2-{}", rah_protocol::SessionId::new()));
    std::fs::create_dir_all(&root).unwrap();
    use rah_runtime_codex::certification_support::measure_artifact;
    let snapshot_path = std::path::Path::new(
        "F:/Temp/rah-codex-certification-snapshot-18440-1791116997708427800-0/codex.exe",
    );
    let before = measure_artifact(snapshot_path).expect("preserved snapshot complete stable read");
    assert_eq!(before.sha256, HASH);
    assert_eq!(before.bytes_read, 326872368);
    assert_eq!(before.file_length_before, 326872368);
    assert_eq!(before.file_length_after, 326872368);
    assert_eq!(before.file_id, Some(0x002e0000000a80a1));
    println!("SNAPSHOT PRE {before:?}");
    let candidate = ExactCodexCandidate {
        path: snapshot_path.into(),
        ..descriptor()
    };

    let mut wrong = candidate.clone();
    wrong.path = root.join("missing-codex.exe");
    let missing_path = wrong.path.clone();
    let error =
        verify_and_construct_candidate(wrong, rah_runtime_codex::CodexModelProvider::OpenAi, &root)
            .await
            .err()
            .unwrap();
    assert!(!error.to_string().contains(missing_path.to_str().unwrap()));
    assert!(
        matches!(verification_cause(&error), CertificationVerificationError::MissingArtifact { path } if path == &missing_path)
    );
    let mut wrong = candidate.clone();
    wrong.expected_sha256 = "0".repeat(64);
    let result =
        verify_and_construct_candidate(wrong, rah_runtime_codex::CodexModelProvider::OpenAi, &root)
            .await;
    let error = result.err().unwrap();
    assert!(
        matches!(verification_cause(&error), CertificationVerificationError::HashMismatch { expected, actual } if expected == &"0".repeat(64) && actual == HASH)
    );
    let mut wrong = candidate.clone();
    wrong.expected_version = "0.157.1".into();
    let result =
        verify_and_construct_candidate(wrong, rah_runtime_codex::CodexModelProvider::OpenAi, &root)
            .await;
    let error = result.err().unwrap();
    assert!(
        matches!(verification_cause(&error), CertificationVerificationError::VersionMismatch { expected, actual } if expected == "0.157.1" && actual == "codex-cli 0.160.0")
    );

    let production = rah_runtime_codex::experimental::CodexFactory::new(
        PATH.into(),
        rah_runtime_codex::CodexModelProvider::OpenAi,
    );
    let result = production.create().await;
    let error = result
        .err()
        .expect("production must reject the same candidate");
    use std::error::Error;
    assert!(
        matches!(error.source().and_then(|s| s.downcast_ref::<rah_runtime_codex::CodexAdapterError>()), Some(rah_runtime_codex::CodexAdapterError::VersionMismatch { actual, .. }) if actual == "codex-cli 0.160.0")
    );
    println!("exact descriptor/wrong path/wrong hash/wrong version/production rejection PASS");

    let factory = verify_and_construct_candidate(
        candidate.clone(),
        rah_runtime_codex::CodexModelProvider::OpenAi,
        &root,
    )
    .await
    .unwrap();
    factory.validate().unwrap();
    // Keep the pre-launch identity immediately adjacent to real composition.
    assert_eq!(measure_artifact(snapshot_path).unwrap(), before);
    {
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(root.clone()))
            .build(tauri::generate_context!())
            .unwrap();
        let state = app.state::<DesktopAppState>();
        // RuntimeDefault avoids selecting or certifying any model.
        tokio::time::timeout(
            Duration::from_secs(60),
            production_composition::connect_with_configuration(
                state.inner(),
                runtime_selection::ProductionAdapter::Codex,
                Some((
                    Box::new(factory),
                    ModelSelection::RuntimeDefault,
                    RuntimeArtifactSource::Path,
                )),
            ),
        )
        .await
        .expect("bounded pre-turn Connect")
        .expect("real Desktop backend Connect");
        let runtime = match &*state.connection.lock().unwrap() {
            ConnectionState::Connected { runtime, .. } => runtime.clone(),
            _ => panic!("no connected runtime"),
        };
        assert!(runtime.is_alive());
        assert_eq!(*state.chat.lock().unwrap(), ChatState::Idle);
        tokio::time::timeout(Duration::from_secs(15), disconnect_codex(state.clone()))
            .await
            .unwrap()
            .unwrap();
        assert!(!runtime.is_alive());
        assert!(matches!(
            *state.connection.lock().unwrap(),
            ConnectionState::NotConnected
        ));
        assert_eq!(measure_artifact(snapshot_path).unwrap(), before);
        assert_eq!(
            rah_runtime_codex::CURRENT_CERTIFIED_CODEX_VERSIONS,
            &["codex-cli 0.157.1"]
        );
        assert_eq!(
            rah_runtime_codex::PREFERRED_CURRENT_CODEX_VERSION,
            "codex-cli 0.157.1"
        );
        println!(
            "real adapter/neutral runtime/Desktop backend/clean shutdown/pre-post hash PASS; inference=0; Tool execution=0"
        );
        drop(runtime);
        // Replace and drop the managed Persistence so its SQLite connection is
        // released before removing the temporary storage root.
        let inert_persistence = Persistence::start(root.parent().unwrap().join(format!(
            "rah-task510b2-persistence-release-{}",
            rah_protocol::SessionId::new()
        )));
        let old_persistence = {
            let mut persistence = state
                .inner()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::replace(&mut *persistence, inert_persistence)
        };
        drop(old_persistence);
    }
    std::fs::remove_dir_all(&root).expect("all fixture persistence owners released");
    assert!(!root.exists());
    assert_eq!(measure_artifact(snapshot_path).unwrap(), before);
    println!(
        "CLEANUP PASS {root:?}; SNAPSHOT POST {:?}",
        measure_artifact(snapshot_path).unwrap()
    );
}

#[tokio::test]
#[ignore = "Task 510B2K explicit snapshot campaign; no cleanup/inference/Tools"]
async fn task510b2k_snapshot_controls() {
    use rah_runtime_codex::certification_support::{create_isolated_snapshot, measure_artifact};
    let frozen = create_isolated_snapshot(&descriptor()).expect("single attempt snapshot creation");
    println!("SOURCE {:?}\nSNAPSHOT {:?}", frozen.source, frozen.snapshot);
    assert_eq!(frozen.source.bytes_read, 326872368);
    let candidate = frozen.candidate();
    assert_ne!(candidate.path, std::path::PathBuf::from(PATH));
    let before = frozen.snapshot.clone();
    let hash = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-FileHash -Algorithm SHA256 -LiteralPath '{}').Hash.ToLower()",
                candidate.path.display()
            ),
        ])
        .output()
        .unwrap();
    assert!(hash.status.success());
    assert_eq!(String::from_utf8_lossy(&hash.stdout).trim(), HASH);
    println!("PowerShell snapshot hash PASS");
    let version = tokio::process::Command::new(&candidate.path)
        .arg("--version")
        .kill_on_drop(true)
        .output()
        .await
        .unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        "codex-cli 0.160.0"
    );
    assert_eq!(measure_artifact(&candidate.path).unwrap(), before);
    println!("snapshot version control PASS exit=0");
    let root = std::env::temp_dir().join(format!(
        "rah-task510b2k-desktop-{}",
        rah_protocol::SessionId::new()
    ));
    std::fs::create_dir(&root).unwrap();
    let factory = verify_and_construct_candidate(
        candidate.clone(),
        rah_runtime_codex::CodexModelProvider::OpenAi,
        &root,
    )
    .await
    .unwrap();
    assert_eq!(measure_artifact(&candidate.path).unwrap(), before);
    let instance = tokio::time::timeout(Duration::from_secs(60), factory.create())
        .await
        .unwrap()
        .unwrap();
    assert!(instance.is_alive());
    instance.shutdown().await.unwrap();
    assert!(!instance.is_alive());
    assert_eq!(measure_artifact(&candidate.path).unwrap(), before);
    println!("real adapter snapshot app-server handshake/shutdown PASS");
    let app = tauri::Builder::default()
        .any_thread()
        .manage(DesktopAppState::new(root.clone()))
        .build(tauri::generate_context!())
        .unwrap();
    let state = app.state::<DesktopAppState>();
    // RuntimeDefault avoids selecting or certifying any model.
    tokio::time::timeout(
        Duration::from_secs(60),
        production_composition::connect_with_configuration(
            state.inner(),
            runtime_selection::ProductionAdapter::Codex,
            Some((
                Box::new(factory),
                ModelSelection::RuntimeDefault,
                RuntimeArtifactSource::Path,
            )),
        ),
    )
    .await
    .expect("bounded pre-turn Connect")
    .expect("real Desktop backend Connect");
    let runtime = match &*state.connection.lock().unwrap() {
        ConnectionState::Connected { runtime, .. } => runtime.clone(),
        _ => panic!("no connected runtime"),
    };
    assert!(runtime.is_alive());
    assert_eq!(*state.chat.lock().unwrap(), ChatState::Idle);
    tokio::time::timeout(Duration::from_secs(15), disconnect_codex(state.clone()))
        .await
        .unwrap()
        .unwrap();
    assert!(!runtime.is_alive());
    assert!(matches!(
        *state.connection.lock().unwrap(),
        ConnectionState::NotConnected
    ));
    assert_eq!(measure_artifact(&candidate.path).unwrap(), before);
    assert_eq!(
        rah_runtime_codex::CURRENT_CERTIFIED_CODEX_VERSIONS,
        &["codex-cli 0.157.1"]
    );
    assert_eq!(
        rah_runtime_codex::PREFERRED_CURRENT_CODEX_VERSION,
        "codex-cli 0.157.1"
    );
    println!(
        "real adapter/neutral runtime/Desktop backend/clean shutdown/pre-post hash PASS; inference=0; Tool execution=0"
    );

    println!(
        "SNAPSHOT FINAL {:?}",
        measure_artifact(&candidate.path).unwrap()
    );
    // Intentionally retain persistence and snapshot roots; no teardown experiment.
}
