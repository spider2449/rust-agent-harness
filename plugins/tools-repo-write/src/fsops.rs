//! Bounded file operations under a canonical repository root.

use std::path::{Path, PathBuf};

use rah_api::tools::ToolError;
use serde::Deserialize;

use crate::undo::{Call, FileState, History, read_state};

pub(crate) struct Root {
    pub path: PathBuf,
    pub max_file_bytes: usize,
    pub locks: crate::locks::Locks,
    /// What the write calls changed, for undo (ADR 0060).
    pub history: History,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Edit {
    path: String,
    old: String,
    new: String,
}

fn invalid(message: impl Into<String>) -> ToolError {
    ToolError::InvalidInput(message.into())
}

/// Accepts `a/b/c`: nonempty components separated by `/`, without `.`,
/// `..`, `.git`, backslashes, colons, NUL or a leading `-`, and not under
/// `.rah/` (see [`is_harness_state`]).
pub(crate) fn validate(path: &str) -> Result<(), ToolError> {
    if is_harness_state(path) {
        return Err(harness_state(path));
    }
    let valid = !path.is_empty()
        && path.len() <= 1024
        && !path.starts_with('-')
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.eq_ignore_ascii_case(".git")
        });
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "`{path}` is not a safe repository-relative path (use `a/b`, no `.`, `..`, `.git`, \
             backslashes, colons or a leading `-`)"
        )))
    }
}

/// Whether `path` is under `.rah/`, where RAH keeps its own state: sessions,
/// memory, skills and `storage` (which holds the Web UI's saved settings,
/// the tool policy among them). A model-requested write there could change
/// what a later start trusts.
pub(crate) fn is_harness_state(path: &str) -> bool {
    path.split(['/', '\\'])
        .find(|component| !component.is_empty() && *component != ".")
        .is_some_and(|first| first.eq_ignore_ascii_case(".rah"))
}

pub(crate) fn harness_state(path: &str) -> ToolError {
    invalid(format!(
        "`{path}` is under `.rah/`, where RAH keeps its own state; the repository tools do not write there"
    ))
}

/// A temporary file removed on drop.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

impl Root {
    fn inside(&self, resolved: &Path, shown: &str) -> Result<(), ToolError> {
        if resolved.starts_with(&self.path) {
            Ok(())
        } else {
            Err(invalid(format!(
                "`{shown}` resolves outside the repository root"
            )))
        }
    }

    /// The canonical path of an existing regular file.
    async fn existing_file(&self, path: &str) -> Result<PathBuf, ToolError> {
        validate(path)?;
        let resolved = tokio::fs::canonicalize(self.path.join(path))
            .await
            .map_err(|_| invalid(format!("`{path}` does not exist")))?;
        self.inside(&resolved, path)?;
        let metadata = tokio::fs::metadata(&resolved)
            .await
            .map_err(|error| invalid(format!("`{path}`: {error}")))?;
        if !metadata.is_file() {
            return Err(invalid(format!("`{path}` is not a regular file")));
        }
        Ok(resolved)
    }

    pub async fn create(
        &self,
        call: &Call<'_>,
        path: &str,
        content: &str,
    ) -> Result<(), ToolError> {
        validate(path)?;
        if content.len() > self.max_file_bytes {
            return Err(invalid(format!(
                "the content exceeds {} bytes",
                self.max_file_bytes
            )));
        }
        let target = self.path.join(path);
        let _writing = self.locks.files(std::slice::from_ref(&target)).await;
        if tokio::fs::symlink_metadata(&target).await.is_ok() {
            return Err(invalid(format!("`{path}` already exists")));
        }
        self.make_parents(&target, path).await?;
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create_new(true);
        let mut file = options
            .open(&target)
            .await
            .map_err(|error| ToolError::Execution(format!("cannot create `{path}`: {error}")))?;
        // A tokio file finishes writes in the background; flush waits for them
        // so the content is on disk when the tool reports success.
        let written = async {
            tokio::io::AsyncWriteExt::write_all(&mut file, content.as_bytes()).await?;
            tokio::io::AsyncWriteExt::flush(&mut file).await
        }
        .await;
        written.map_err(|error| ToolError::Execution(format!("cannot write `{path}`: {error}")))?;
        self.history.record(
            call,
            vec![FileState {
                path: path.to_owned(),
                before: None,
                after: Some(content.as_bytes().to_vec()),
            }],
        );
        Ok(())
    }

    /// Writes `content` back to `target` for an undo, through a temporary
    /// file and a rename, creating missing directories above it.
    pub async fn write_back(
        &self,
        target: &Path,
        shown: &str,
        content: &[u8],
    ) -> Result<(), String> {
        self.make_parents(target, shown)
            .await
            .map_err(|error| error.to_string())?;
        let temporary = temporary_beside(target);
        let result = async {
            tokio::fs::write(&temporary, content).await?;
            tokio::fs::rename(&temporary, target).await
        }
        .await;
        if let Err(error) = result {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err(format!("cannot write `{shown}`: {error}"));
        }
        Ok(())
    }

    /// Creates the missing directories above `target`. The deepest existing
    /// ancestor must be inside the root, so creating them cannot follow a
    /// link out of it.
    async fn make_parents(&self, target: &Path, shown: &str) -> Result<(), ToolError> {
        let mut ancestor = target.parent().map(Path::to_path_buf).unwrap_or_default();
        while tokio::fs::symlink_metadata(&ancestor).await.is_err() {
            match ancestor.parent() {
                Some(parent) => ancestor = parent.to_path_buf(),
                None => break,
            }
        }
        let resolved = tokio::fs::canonicalize(&ancestor)
            .await
            .map_err(|error| invalid(format!("`{shown}`: {error}")))?;
        self.inside(&resolved, shown)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                ToolError::Execution(format!("cannot create the directory: {error}"))
            })?;
        }
        Ok(())
    }

    /// The canonical path of a regular file that is not a symbolic link: the
    /// delete and move tools act on the file itself, never on what a link
    /// points to.
    async fn plain_file(&self, path: &str) -> Result<PathBuf, ToolError> {
        validate(path)?;
        let metadata = tokio::fs::symlink_metadata(self.path.join(path))
            .await
            .map_err(|_| invalid(format!("`{path}` does not exist")))?;
        if metadata.file_type().is_symlink() {
            return Err(invalid(format!(
                "`{path}` is a symbolic link; this tool does not change links"
            )));
        }
        if !metadata.is_file() {
            return Err(invalid(format!("`{path}` is not a regular file")));
        }
        self.existing_file(path).await
    }

    /// Deletes one regular file.
    pub async fn delete(&self, call: &Call<'_>, path: &str) -> Result<(), ToolError> {
        let resolved = self.plain_file(path).await?;
        let _writing = self.locks.files(std::slice::from_ref(&resolved)).await;
        // The file may have changed while the lock was awaited.
        if self.plain_file(path).await? != resolved {
            return Err(invalid(format!("`{path}` changed while it was locked")));
        }
        // A file too large to keep is deleted without an undo.
        let kept = read_state(&resolved, self.max_file_bytes).await;
        tokio::fs::remove_file(&resolved)
            .await
            .map_err(|error| ToolError::NotStarted(format!("cannot delete `{path}`: {error}")))?;
        if let Ok(Some(content)) = kept {
            self.history.record(
                call,
                vec![FileState {
                    path: path.to_owned(),
                    before: Some(content),
                    after: None,
                }],
            );
        }
        Ok(())
    }

    /// Renames one regular file to a path that does not exist yet, creating
    /// the missing directories above it.
    pub async fn rename(&self, call: &Call<'_>, from: &str, to: &str) -> Result<(), ToolError> {
        let source = self.plain_file(from).await?;
        validate(to)?;
        if from == to {
            return Err(invalid("`from` and `to` are the same path"));
        }
        let target = self.path.join(to);
        let _writing = self.locks.files(&[source.clone(), target.clone()]).await;
        if self.plain_file(from).await? != source {
            return Err(invalid(format!("`{from}` changed while it was locked")));
        }
        if tokio::fs::symlink_metadata(&target).await.is_ok() {
            return Err(invalid(format!("`{to}` already exists")));
        }
        self.make_parents(&target, to).await?;
        let kept = read_state(&source, self.max_file_bytes).await;
        tokio::fs::rename(&source, &target).await.map_err(|error| {
            ToolError::NotStarted(format!("cannot move `{from}` to `{to}`: {error}"))
        })?;
        if let Ok(Some(content)) = kept {
            self.history.record(
                call,
                vec![
                    FileState {
                        path: from.to_owned(),
                        before: Some(content.clone()),
                        after: None,
                    },
                    FileState {
                        path: to.to_owned(),
                        before: None,
                        after: Some(content),
                    },
                ],
            );
        }
        Ok(())
    }

    /// Checks every edit, then writes every changed file. Returns the paths.
    pub async fn edit(&self, call: &Call<'_>, edits: Vec<Edit>) -> Result<Vec<String>, ToolError> {
        // Resolve every file first, then hold their locks while they are read,
        // edited and written, so a concurrent edit is not overwritten.
        let mut resolved_paths: Vec<(String, PathBuf)> = Vec::new();
        for edit in &edits {
            if !resolved_paths.iter().any(|(path, _)| *path == edit.path) {
                resolved_paths.push((edit.path.clone(), self.existing_file(&edit.path).await?));
            }
        }
        let locked: Vec<PathBuf> = resolved_paths
            .iter()
            .map(|(_, resolved)| resolved.clone())
            .collect();
        let _writing = self.locks.files(&locked).await;
        // path -> (resolved, new content), in first-seen order, and the
        // content before, for undo.
        let mut files: Vec<(String, PathBuf, String)> = Vec::new();
        let mut originals: Vec<String> = Vec::new();
        for edit in edits {
            let index = match files.iter().position(|(path, ..)| *path == edit.path) {
                Some(index) => index,
                None => {
                    let resolved = resolved_paths
                        .iter()
                        .find(|(path, _)| *path == edit.path)
                        .map(|(_, resolved)| resolved.clone())
                        .ok_or_else(|| invalid("an edit names a file that was not resolved"))?;
                    let bytes = tokio::fs::read(&resolved)
                        .await
                        .map_err(|error| invalid(format!("`{}`: {error}", edit.path)))?;
                    let text = String::from_utf8(bytes)
                        .map_err(|_| invalid(format!("`{}` is not UTF-8 text", edit.path)))?;
                    originals.push(text.clone());
                    files.push((edit.path.clone(), resolved, text));
                    files.len() - 1
                }
            };
            let text = &mut files[index].2;
            *text = replace_once(text, &edit.old, &edit.new)
                .map_err(|problem| invalid(format!("`{}`: {problem}", edit.path)))?;
            if text.len() > self.max_file_bytes {
                return Err(invalid(format!(
                    "`{}` would exceed {} bytes",
                    edit.path, self.max_file_bytes
                )));
            }
        }
        for (written, (path, resolved, text)) in files.iter().enumerate() {
            let temporary = temporary_beside(resolved);
            let result = async {
                tokio::fs::write(&temporary, text.as_bytes()).await?;
                tokio::fs::rename(&temporary, resolved).await
            }
            .await;
            if let Err(error) = result {
                let _ = tokio::fs::remove_file(&temporary).await;
                let error = format!("cannot write `{path}`: {error}");
                return Err(if written == 0 {
                    ToolError::NotStarted(error)
                } else {
                    ToolError::Execution(format!("{error}; earlier files were already written"))
                });
            }
        }
        // A file too large to keep leaves the call without an undo.
        if originals
            .iter()
            .all(|text| text.len() <= self.max_file_bytes)
        {
            self.history.record(
                call,
                files
                    .iter()
                    .zip(originals)
                    .map(|((path, _, text), original)| FileState {
                        path: path.clone(),
                        before: Some(original.into_bytes()),
                        after: Some(text.as_bytes().to_vec()),
                    })
                    .collect(),
            );
        }
        Ok(files.into_iter().map(|(path, ..)| path).collect())
    }

    /// Writes `content` to a temporary file outside the repository.
    pub async fn scratch_file(&self, content: &str) -> Result<Scratch, ToolError> {
        let path =
            std::env::temp_dir().join(format!("rah-patch-{}.diff", uuid::Uuid::new_v4().simple()));
        tokio::fs::write(&path, content)
            .await
            .map_err(|error| ToolError::NotStarted(format!("cannot stage the patch: {error}")))?;
        Ok(Scratch(path))
    }
}

/// A new temporary file name in the directory of `file`.
fn temporary_beside(file: &Path) -> PathBuf {
    file.with_file_name(format!(
        ".{}.rah-{}",
        file.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        uuid::Uuid::new_v4().simple()
    ))
}

/// Replaces the single occurrence of `old`. Matches CRLF files when `old`
/// uses LF line endings.
pub(crate) fn replace_once(text: &str, old: &str, new: &str) -> Result<String, String> {
    let attempt = |old: &str, new: &str| match text.matches(old).count() {
        1 => Ok(text.replacen(old, new, 1)),
        0 => Err(0),
        count => Err(count),
    };
    match attempt(old, new) {
        Ok(text) => Ok(text),
        Err(0) if text.contains("\r\n") && old.contains('\n') && !old.contains("\r\n") => {
            attempt(&old.replace('\n', "\r\n"), &new.replace('\n', "\r\n")).map_err(|count| {
                if count == 0 {
                    "`old` was not found".to_owned()
                } else {
                    format!("`old` occurs {count} times; make it unique")
                }
            })
        }
        Err(0) => Err("`old` was not found".to_owned()),
        Err(count) => Err(format!("`old` occurs {count} times; make it unique")),
    }
}
