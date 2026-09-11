use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{ToolContent, ToolOutput};
use rah_tools::{
    RepositoryFileRenameTool, RepositoryRenameFilePreparationError,
    RepositoryRenameFilePreparationRequest, RepositoryRenameFilePreparer,
    RepositoryRenameFileProof, Tool, ToolContext, classify_repository_rename_file_output,
};
use serde_json::json;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    git: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rah-rename-preparation-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("nested")).unwrap();
        let git = native_git();
        run(&git, &root, &["init", "--quiet"]);
        run(&git, &root, &["config", "user.name", "RAH Test"]);
        run(
            &git,
            &root,
            &["config", "user.email", "rah@example.invalid"],
        );
        fs::write(root.join("source.txt"), b"rename source\n").unwrap();
        run(&git, &root, &["add", "."]);
        run(&git, &root, &["commit", "--quiet", "-m", "base"]);
        Self { root, git }
    }

    fn commit_source(&self, bytes: &[u8]) {
        fs::write(self.root.join("source.txt"), bytes).unwrap();
        run(&self.git, &self.root, &["add", "source.txt"]);
        run(
            &self.git,
            &self.root,
            &["commit", "--quiet", "-m", "source"],
        );
    }

    fn snapshot(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let refs = Command::new(&self.git)
            .args(["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
            .current_dir(&self.root)
            .output()
            .unwrap()
            .stdout;
        (
            fs::read(self.root.join("source.txt")).unwrap_or_default(),
            fs::read(self.root.join(".git/index")).unwrap(),
            refs,
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

fn prepare(
    preparer: &RepositoryRenameFilePreparer,
    source_path: &str,
    destination_path: &str,
) -> Result<rah_tools::RepositoryRenameFilePreparation, RepositoryRenameFilePreparationError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.prepare(RepositoryRenameFilePreparationRequest {
        source_path: source_path.to_owned(),
        destination_path: destination_path.to_owned(),
    }))
}

fn revalidate(
    preparer: &RepositoryRenameFilePreparer,
    preparation: &rah_tools::RepositoryRenameFilePreparation,
) -> Result<(), RepositoryRenameFilePreparationError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.revalidate(preparation))
}

fn prove_result(
    preparer: &RepositoryRenameFilePreparer,
    preparation: &rah_tools::RepositoryRenameFilePreparation,
    output: &ToolOutput,
) -> RepositoryRenameFileProof {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.prove_result(preparation, output))
}

fn json_output(value: serde_json::Value, is_error: bool) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error,
    }
}

#[test]
fn same_directory_prepare_is_complete_and_zero_effect() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "renamed.txt").unwrap();
    assert_eq!(preparation.review().operation(), "repo.rename-file");
    assert_eq!(preparation.review().source_path(), "source.txt");
    assert_eq!(preparation.review().destination_path(), "renamed.txt");
    assert!(
        preparation
            .review()
            .source_content_escaped()
            .contains("rename")
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn cross_directory_prepare_and_host_derived_tool_input_succeed() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "nested/renamed.txt").unwrap();
    assert_eq!(
        preparation.tool_input().0,
        json!({
            "source_path": "source.txt",
            "destination_path": "nested/renamed.txt",
            "expected_source_file_sha256": preparation.source_sha256(),
            "expected_source_file_byte_length": preparation.source_byte_length(),
        })
    );
    assert!(revalidate(&preparer, &preparation).is_ok());
}

#[test]
fn ordinary_schema_is_unchanged_and_reviewed_route_has_no_tool_dispatch() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let _ = prepare(&preparer, "source.txt", "renamed.txt").unwrap();
    let tool = RepositoryFileRenameTool::new(&fixture.git, &fixture.root).unwrap();
    assert_eq!(
        tool.definition().input_schema,
        json!({"type":"object","properties":{"source_path":{"type":"string","minLength":1,"maxLength":1024},"destination_path":{"type":"string","minLength":1,"maxLength":1024},"expected_source_file_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_source_file_byte_length":{"type":"integer","minimum":0,"maximum":1024*1024}},"required":["source_path","destination_path","expected_source_file_sha256","expected_source_file_byte_length"],"additionalProperties":false})
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn invalid_closed_requests_fail_before_observation() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    for (source, destination) in [
        ("", "new.txt"),
        ("source.txt", ""),
        ("../source.txt", "new.txt"),
        ("source.txt", ".git/new.txt"),
        ("source.txt", "new\\file.txt"),
        ("source.txt", "source.txt"),
    ] {
        assert!(matches!(
            prepare(&preparer, source, destination),
            Err(RepositoryRenameFilePreparationError::InvalidInput { .. })
        ));
    }
}

#[test]
fn oversized_source_is_rejected() {
    let fixture = Fixture::new();
    fixture.commit_source(&vec![b'a'; 65_537]);
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(matches!(
        prepare(&preparer, "source.txt", "new.txt"),
        Err(RepositoryRenameFilePreparationError::PreconditionFailed { .. })
    ));
}

#[test]
fn non_utf8_and_nul_sources_are_rejected() {
    let fixture = Fixture::new();
    fixture.commit_source(&[0xff, 0xfe]);
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "new.txt").is_err());
    fixture.commit_source(b"nul\0content");
    assert!(prepare(&preparer, "source.txt", "new.txt").is_err());
}

#[test]
fn untracked_dirty_and_staged_sources_are_rejected() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    fs::write(fixture.root.join("untracked.txt"), b"untracked").unwrap();
    assert!(prepare(&preparer, "untracked.txt", "new.txt").is_err());
    fs::write(fixture.root.join("source.txt"), b"dirty").unwrap();
    assert!(prepare(&preparer, "source.txt", "new.txt").is_err());
    run(&fixture.git, &fixture.root, &["restore", "source.txt"]);
    fs::write(fixture.root.join("source.txt"), b"staged").unwrap();
    run(&fixture.git, &fixture.root, &["add", "source.txt"]);
    assert!(prepare(&preparer, "source.txt", "new.txt").is_err());
}

#[cfg(unix)]
#[test]
fn unsupported_mode_is_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let mut permissions = fs::metadata(fixture.root.join("source.txt"))
        .unwrap()
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(fixture.root.join("source.txt"), permissions).unwrap();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "new.txt").is_err());
}

#[test]
fn destination_parent_and_collisions_are_rejected() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    fs::write(fixture.root.join("existing.txt"), b"existing").unwrap();
    assert!(prepare(&preparer, "source.txt", "existing.txt").is_err());
    assert!(prepare(&preparer, "source.txt", "missing/new.txt").is_err());
    run(&fixture.git, &fixture.root, &["add", "existing.txt"]);
    assert!(prepare(&preparer, "source.txt", "existing.txt").is_err());
}

#[test]
fn ignored_destination_is_rejected() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join(".gitignore"), "ignored/\n").unwrap();
    fs::create_dir_all(fixture.root.join("ignored")).unwrap();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "ignored/new.txt").is_err());
}

#[test]
fn nested_repository_boundaries_are_rejected() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.root.join("nested/sub")).unwrap();
    run(
        &fixture.git,
        &fixture.root.join("nested"),
        &["init", "--quiet"],
    );
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "nested/sub/new.txt").is_err());
}

#[cfg(unix)]
#[test]
fn symlink_ancestry_is_rejected() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    symlink(fixture.root.join("nested"), fixture.root.join("alias")).unwrap();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "alias/new.txt").is_err());
}

#[test]
fn review_contains_complete_escaped_source_and_non_effects() {
    let fixture = Fixture::new();
    fixture.commit_source("a \t\r\n\\\" λ\u{7f}".as_bytes());
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    let review = preparation.review();
    assert!(review.source_content_escaped().contains("\\r"));
    assert!(review.source_content_escaped().contains("\\u{3bb}"));
    assert!(review.source_content_escaped().contains("\\u{7f}"));
    for non_effect in [
        "no content rewrite",
        "no staging",
        "no unstaging",
        "no commit",
        "no branch/ref/history mutation",
        "no directory creation",
        "no overwrite",
        "no import/reference rewrite",
        "no automatic retry",
        "no replay",
        "no rollback",
        "no compensation",
    ] {
        assert!(review.non_effects().contains(&non_effect));
    }
}

#[test]
fn review_size_overflow_fails_closed() {
    let fixture = Fixture::new();
    fixture.commit_source(&vec![0x7f; 65_536]);
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(matches!(
        prepare(&preparer, "source.txt", "new.txt"),
        Err(RepositoryRenameFilePreparationError::ReviewTooLarge)
    ));
}

#[test]
fn unchanged_preparation_revalidates() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    assert!(revalidate(&preparer, &preparation).is_ok());
}

#[test]
fn same_bytes_with_new_identity_is_stale() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    let bytes = fs::read(fixture.root.join("source.txt")).unwrap();
    fs::rename(
        fixture.root.join("source.txt"),
        fixture.root.join("old.txt"),
    )
    .unwrap();
    fs::write(fixture.root.join("source.txt"), bytes).unwrap();
    assert!(matches!(
        revalidate(&preparer, &preparation),
        Err(RepositoryRenameFilePreparationError::Stale)
    ));
}

#[test]
fn content_parent_destination_and_nested_drift_are_stale() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    fs::write(fixture.root.join("source.txt"), b"drift").unwrap();
    assert!(revalidate(&preparer, &preparation).is_err());
    fs::write(fixture.root.join("source.txt"), b"rename source\n").unwrap();
    fs::write(fixture.root.join(".gitignore"), "new.txt\n").unwrap();
    assert!(matches!(
        revalidate(&preparer, &preparation),
        Err(RepositoryRenameFilePreparationError::Stale)
    ));
    fs::remove_file(fixture.root.join(".gitignore")).unwrap();
    fs::write(fixture.root.join("new.txt"), b"collision").unwrap();
    assert!(matches!(
        revalidate(&preparer, &preparation),
        Err(RepositoryRenameFilePreparationError::Stale)
    ));
}

#[test]
fn exact_preimage_and_absent_destination_prove_known_no_effect() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    let output = json_output(json!({"status":"known_no_effect","uncertain":false}), true);
    assert_eq!(
        prove_result(&preparer, &preparation, &output),
        RepositoryRenameFileProof::KnownNoEffect
    );
}

#[test]
fn success_requires_independent_filesystem_proof() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    let success = json_output(
        json!({"status":"renamed_verified","uncertain":false,"path":"new.txt"}),
        false,
    );
    assert_eq!(
        prove_result(&preparer, &preparation, &success),
        RepositoryRenameFileProof::Uncertain
    );
    let tool = RepositoryFileRenameTool::new(&fixture.git, &fixture.root).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let output = runtime
        .block_on(tool.execute(preparation.tool_input().clone(), ToolContext::default()))
        .unwrap();
    assert_eq!(
        prove_result(&preparer, &preparation, &output),
        RepositoryRenameFileProof::ReviewedSuccess
    );
}

#[test]
fn same_bytes_wrong_identity_does_not_prove_no_effect() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    let bytes = fs::read(fixture.root.join("source.txt")).unwrap();
    fs::rename(
        fixture.root.join("source.txt"),
        fixture.root.join("old.txt"),
    )
    .unwrap();
    fs::write(fixture.root.join("source.txt"), bytes).unwrap();
    let output = json_output(json!({"status":"known_no_effect","uncertain":false}), true);
    assert_eq!(
        prove_result(&preparer, &preparation, &output),
        RepositoryRenameFileProof::Uncertain
    );
}

#[test]
fn malformed_output_is_uncertain_and_no_recovery_is_attempted() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    for output in [
        ToolOutput {
            content: vec![],
            is_error: false,
        },
        json_output(
            json!({"status":"renamed_verified","uncertain":false,"path":"wrong.txt"}),
            false,
        ),
        json_output(json!({"status":"unknown","uncertain":false}), true),
        json_output(json!({"status":"uncertain","uncertain":false}), true),
    ] {
        assert_eq!(
            prove_result(&preparer, &preparation, &output),
            RepositoryRenameFileProof::Uncertain
        );
    }
    assert!(fixture.root.join("source.txt").exists());
    assert!(!fixture.root.join("new.txt").exists());
}

#[test]
fn output_classifier_rejects_extensions_and_accepts_only_current_shapes() {
    let success = json_output(
        json!({"status":"renamed_verified","uncertain":false,"path":"new.txt"}),
        false,
    );
    assert_eq!(
        classify_repository_rename_file_output(&success, "new.txt"),
        RepositoryRenameFileProof::ReviewedSuccess
    );
    let extra = json_output(
        json!({"status":"renamed_verified","uncertain":false,"path":"new.txt","extra":1}),
        false,
    );
    assert_eq!(
        classify_repository_rename_file_output(&extra, "new.txt"),
        RepositoryRenameFileProof::Uncertain
    );
    let no_effect = json_output(json!({"status":"known_no_effect","uncertain":false}), true);
    assert_eq!(
        classify_repository_rename_file_output(&no_effect, "new.txt"),
        RepositoryRenameFileProof::KnownNoEffect
    );
}

#[cfg(unix)]
#[test]
fn source_link_count_is_rejected() {
    let fixture = Fixture::new();
    fs::hard_link(
        fixture.root.join("source.txt"),
        fixture.root.join("source-link.txt"),
    )
    .unwrap();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "new.txt").is_err());
}

#[cfg(windows)]
#[test]
fn windows_case_only_alias_is_rejected_before_effect() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "SOURCE.TXT").is_err());
}

#[test]
fn destination_head_collision_is_rejected_even_when_worktree_is_absent() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("destination.txt"), b"destination").unwrap();
    run(&fixture.git, &fixture.root, &["add", "destination.txt"]);
    run(
        &fixture.git,
        &fixture.root,
        &["commit", "--quiet", "-m", "destination"],
    );
    fs::remove_file(fixture.root.join("destination.txt")).unwrap();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    assert!(prepare(&preparer, "source.txt", "destination.txt").is_err());
}

#[test]
fn index_drift_is_stale() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "new.txt").unwrap();
    fs::write(fixture.root.join("unrelated.txt"), b"unrelated").unwrap();
    run(&fixture.git, &fixture.root, &["add", "unrelated.txt"]);
    assert!(revalidate(&preparer, &preparation).is_err());
}

#[test]
fn nested_boundary_appearing_after_prepare_is_stale() {
    let fixture = Fixture::new();
    let preparer = RepositoryRenameFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let preparation = prepare(&preparer, "source.txt", "nested/new.txt").unwrap();
    run(
        &fixture.git,
        &fixture.root.join("nested"),
        &["init", "--quiet"],
    );
    assert!(matches!(
        revalidate(&preparer, &preparation),
        Err(RepositoryRenameFilePreparationError::Stale)
    ));
}
