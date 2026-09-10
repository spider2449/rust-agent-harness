use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{ToolContent, ToolInput};
use rah_tools::{
    RepositoryDeleteFilePreparationError, RepositoryDeleteFilePreparationRequest,
    RepositoryDeleteFilePreparer, RepositoryFileDeletionTool, Tool, ToolContext,
};
use serde_json::{Value, json};
use sha2::Digest as _;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);
type Snapshot = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

struct Fixture {
    root: PathBuf,
    git: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rah-delete-preparation-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let git = native_git();
        run(&git, &root, &["init", "--quiet"]);
        run(&git, &root, &["config", "core.autocrlf", "false"]);
        run(&git, &root, &["config", "user.name", "RAH Test"]);
        run(
            &git,
            &root,
            &["config", "user.email", "rah@example.invalid"],
        );
        fs::write(root.join("target.txt"), b"protected\n").unwrap();
        fs::write(root.join("sentinel.txt"), b"untouched\n").unwrap();
        run(&git, &root, &["add", "."]);
        run(&git, &root, &["commit", "--quiet", "-m", "base"]);
        Self { root, git }
    }

    fn commit_bytes(&self, path: &str, bytes: &[u8], message: &str) {
        let native = self.root.join(path);
        if let Some(parent) = native.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(native, bytes).unwrap();
        run(&self.git, &self.root, &["add", "--", path]);
        run(&self.git, &self.root, &["commit", "--quiet", "-m", message]);
    }

    fn snapshot(&self) -> Snapshot {
        (
            fs::read(self.root.join("target.txt")).unwrap_or_default(),
            fs::read(self.root.join("sentinel.txt")).unwrap(),
            fs::read(self.root.join(".git/HEAD")).unwrap(),
            fs::read(self.root.join(".git/index")).unwrap(),
            refs(&self.git, &self.root),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn native_git() -> PathBuf {
    #[cfg(windows)]
    let command = "where.exe";
    #[cfg(not(windows))]
    let command = "which";
    #[cfg(windows)]
    let argument = "git.exe";
    #[cfg(not(windows))]
    let argument = "git";
    let output = Command::new(command).arg(argument).output().unwrap();
    fs::canonicalize(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap()
}

fn run(git: &Path, root: &Path, args: &[&str]) {
    assert!(
        Command::new(git)
            .args(args)
            .current_dir(root)
            .status()
            .unwrap()
            .success(),
        "{args:?}"
    );
}

fn refs(git: &Path, root: &Path) -> Vec<u8> {
    Command::new(git)
        .args(["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
        .current_dir(root)
        .output()
        .unwrap()
        .stdout
}

fn prepare(
    preparer: &RepositoryDeleteFilePreparer,
    path: &str,
) -> Result<rah_tools::RepositoryDeleteFilePreparation, RepositoryDeleteFilePreparationError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.prepare(RepositoryDeleteFilePreparationRequest {
        path: path.to_owned(),
    }))
}

fn revalidate(
    preparer: &RepositoryDeleteFilePreparer,
    preparation: &rah_tools::RepositoryDeleteFilePreparation,
) -> Result<(), RepositoryDeleteFilePreparationError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.revalidate(preparation))
}

fn prove_known_no_effect(
    preparer: &RepositoryDeleteFilePreparer,
    preparation: &rah_tools::RepositoryDeleteFilePreparation,
) -> bool {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.prove_known_no_effect(preparation))
}

fn prove_deleted_verified(
    preparer: &RepositoryDeleteFilePreparer,
    preparation: &rah_tools::RepositoryDeleteFilePreparation,
) -> bool {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.prove_deleted_verified(preparation))
}

fn execute(tool: &RepositoryFileDeletionTool, input: Value) -> Value {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let output = runtime
        .block_on(tool.execute(ToolInput(input), ToolContext::default()))
        .unwrap();
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("JSON result required")
    };
    value.clone()
}

#[test]
fn prepares_exact_tool_input_and_complete_review_without_effect() {
    let fixture = Fixture::new();
    let content = "\u{feff}a \r\n\t\"\\ λ\u{200b}\n";
    fixture.commit_bytes("target.txt", content.as_bytes(), "review content");
    let before = fixture.snapshot();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();

    assert_eq!(
        preparation.tool_input().0,
        json!({
            "path": "target.txt",
            "expected_file_sha256": format!("{:x}", sha2::Sha256::digest(content.as_bytes())),
            "expected_file_byte_length": content.len()
        })
    );
    assert_eq!(preparation.review().operation(), "repo.delete-file");
    assert_eq!(preparation.review().target_count(), 1);
    assert_eq!(preparation.review().path(), "target.txt");
    assert_eq!(
        preparation.review().tracked_state(),
        "clean HEAD-tracked stage-0 regular file"
    );
    assert_eq!(preparation.review().file_mode(), "100644");
    assert_eq!(
        preparation.review().preimage(),
        r#"\u{feff}a\u{20}\r\n\t\"\\\u{20}\u{3bb}\u{200b}\n"#
    );
    assert_eq!(preparation.review().content_byte_length(), content.len());
    assert_eq!(
        preparation.review().bom(),
        rah_tools::RepositoryDeleteFileBomState::Present
    );
    assert!(preparation.review().content_facts().contains_crlf);
    assert!(preparation.review().content_facts().contains_tab);
    assert!(preparation.review().content_facts().contains_trailing_space);
    assert!(
        preparation
            .review()
            .content_facts()
            .contains_control_or_format_escape
    );
    assert!(
        preparation
            .review()
            .warnings()
            .iter()
            .any(|warning| warning.contains("no backup"))
    );
    assert!(preparation.review().non_effects().contains(&"not Commit"));
    assert!(serde_json::to_vec(preparation.review()).unwrap().len() <= 262144);
    assert!(revalidate(&preparer, &preparation).is_ok());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn preparation_and_revalidation_have_zero_effect_and_tool_schema_is_unchanged() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    assert!(fixture.root.join("target.txt").exists());
    assert_eq!(fixture.snapshot(), before);
    assert!(revalidate(&preparer, &preparation).is_ok());
    assert_eq!(fixture.snapshot(), before);

    let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
    assert_eq!(
        tool.definition().input_schema,
        json!({"type":"object","properties":{"path":{"type":"string","minLength":1,"maxLength":1024},"expected_file_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_file_byte_length":{"type":"integer","minimum":0,"maximum":1024*1024}},"required":["path","expected_file_sha256","expected_file_byte_length"],"additionalProperties":false})
    );
}

#[test]
fn reviewed_postcondition_proofs_are_independent_and_identity_bound() {
    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    assert!(prove_known_no_effect(&preparer, &preparation));

    let original = fs::read(fixture.root.join("target.txt")).unwrap();
    fs::rename(
        fixture.root.join("target.txt"),
        fixture.root.join("displaced.txt"),
    )
    .unwrap();
    fs::write(fixture.root.join("target.txt"), &original).unwrap();
    assert!(!prove_known_no_effect(&preparer, &preparation));

    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
    let output = execute(
        &tool,
        json!({
            "path": "target.txt",
            "expected_file_sha256": format!("{:x}", sha2::Sha256::digest(b"protected\n")),
            "expected_file_byte_length": 10
        }),
    );
    assert_eq!(output["status"], "deleted_verified");
    assert!(prove_deleted_verified(&preparer, &preparation));
}

#[test]
fn different_preparer_rejects_preparation() {
    let fixture = Fixture::new();
    let first = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let second = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&first, "target.txt").unwrap();
    assert_eq!(
        revalidate(&second, &preparation),
        Err(RepositoryDeleteFilePreparationError::Stale)
    );
}

#[test]
fn source_identity_and_missing_target_drift_are_stale() {
    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    fs::rename(
        fixture.root.join("target.txt"),
        fixture.root.join("replacement-source.txt"),
    )
    .unwrap();
    fs::write(fixture.root.join("target.txt"), b"protected\n").unwrap();
    assert_eq!(
        revalidate(&preparer, &preparation),
        Err(RepositoryDeleteFilePreparationError::Stale)
    );

    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    fs::remove_file(fixture.root.join("target.txt")).unwrap();
    assert_eq!(
        revalidate(&preparer, &preparation),
        Err(RepositoryDeleteFilePreparationError::Stale)
    );
}

#[test]
fn source_index_head_and_ref_drift_are_stale() {
    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    fs::write(fixture.root.join("target.txt"), b"staged change\n").unwrap();
    run(&fixture.git, &fixture.root, &["add", "target.txt"]);
    assert_eq!(
        revalidate(&preparer, &preparation),
        Err(RepositoryDeleteFilePreparationError::Stale)
    );

    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    fixture.commit_bytes("head-drift.txt", b"head drift\n", "head drift");
    assert_eq!(
        revalidate(&preparer, &preparation),
        Err(RepositoryDeleteFilePreparationError::Stale)
    );

    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    run(&fixture.git, &fixture.root, &["branch", "unrelated-ref"]);
    assert_eq!(
        revalidate(&preparer, &preparation),
        Err(RepositoryDeleteFilePreparationError::Stale)
    );
}

#[test]
fn reviewed_route_rejects_binary_nul_and_oversized_sources_but_ordinary_tool_remains_broader() {
    let fixture = Fixture::new();
    fixture.commit_bytes("target.txt", &[0, 0xff, 1], "binary target");
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(matches!(
        prepare(&preparer, "target.txt"),
        Err(RepositoryDeleteFilePreparationError::PreconditionFailed { .. })
    ));

    let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
    let binary = [0, 0xff, 1];
    let ordinary = execute(
        &tool,
        json!({"path":"target.txt","expected_file_sha256":format!("{:x}", sha2::Sha256::digest(binary)),"expected_file_byte_length":binary.len()}),
    );
    assert_eq!(ordinary["status"], "deleted_verified");

    let fixture = Fixture::new();
    fixture.commit_bytes("target.txt", b"a\0b", "nul target");
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(matches!(
        prepare(&preparer, "target.txt"),
        Err(RepositoryDeleteFilePreparationError::PreconditionFailed { .. })
    ));

    let fixture = Fixture::new();
    fixture.commit_bytes("target.txt", &vec![b'x'; 65537], "oversized target");
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(matches!(
        prepare(&preparer, "target.txt"),
        Err(RepositoryDeleteFilePreparationError::PreconditionFailed { .. })
    ));
}

#[test]
fn empty_and_exactly_65536_byte_sources_are_supported() {
    let fixture = Fixture::new();
    fixture.commit_bytes("target.txt", b"", "empty target");
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let empty = prepare(&preparer, "target.txt").unwrap();
    assert!(empty.review().content_facts().empty);
    assert_eq!(empty.review().preimage(), "");

    let fixture = Fixture::new();
    fixture.commit_bytes("target.txt", &vec![b'x'; 65536], "maximum target");
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let maximum = prepare(&preparer, "target.txt").unwrap();
    assert_eq!(maximum.review().content_byte_length(), 65536);
    assert!(revalidate(&preparer, &maximum).is_ok());
}

#[test]
fn escaped_review_overflow_fails_without_preparation() {
    let fixture = Fixture::new();
    let source = vec![1_u8; 65536];
    fixture.commit_bytes("target.txt", &source, "review overflow");
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(matches!(
        prepare(&preparer, "target.txt"),
        Err(RepositoryDeleteFilePreparationError::ReviewTooLarge)
    ));
    assert_eq!(fs::read(fixture.root.join("target.txt")).unwrap(), source);
}

#[test]
fn review_reports_supported_modes_and_private_surfaces_are_redacted() {
    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "target.txt").unwrap();
    let debug = format!("{preparation:?}");
    assert!(!debug.contains("protected"));
    assert!(!debug.contains("target.txt"));
    assert!(!debug.contains(preparation.review().content_sha256()));
    assert!(!debug.contains(&preparation.review().content_byte_length().to_string()));
    assert!(!format!("{preparation:?}").contains(fixture.root.to_string_lossy().as_ref()));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let fixture = Fixture::new();
        let target = fixture.root.join("target.txt");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o755)).unwrap();
        run(&fixture.git, &fixture.root, &["add", "target.txt"]);
        run(
            &fixture.git,
            &fixture.root,
            &["commit", "--quiet", "-m", "mode"],
        );
        let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
        assert_eq!(
            prepare(&preparer, "target.txt")
                .unwrap()
                .review()
                .file_mode(),
            "100755"
        );
    }
}

#[test]
fn invalid_path_errors_are_bounded_and_do_not_leak_values() {
    let fixture = Fixture::new();
    let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let request = RepositoryDeleteFilePreparationRequest {
        path: "../secret-value.txt".to_owned(),
    };
    let error = {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(preparer.prepare(request)).unwrap_err()
    };
    assert!(matches!(
        error,
        RepositoryDeleteFilePreparationError::InvalidInput { .. }
    ));
    assert!(!error.to_string().contains("secret-value"));
    assert!(
        !error
            .to_string()
            .contains(fixture.root.to_string_lossy().as_ref())
    );
}
