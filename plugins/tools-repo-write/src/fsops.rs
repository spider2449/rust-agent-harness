//! Bounded file operations under a canonical repository root.

use std::path::{Path, PathBuf};

use rah_api::tools::ToolError;
use serde::Deserialize;

pub(crate) struct Root {
    pub path: PathBuf,
    pub max_file_bytes: usize,
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

    pub async fn create(&self, path: &str, content: &str) -> Result<(), ToolError> {
        validate(path)?;
        if content.len() > self.max_file_bytes {
            return Err(invalid(format!(
                "the content exceeds {} bytes",
                self.max_file_bytes
            )));
        }
        let target = self.path.join(path);
        if tokio::fs::symlink_metadata(&target).await.is_ok() {
            return Err(invalid(format!("`{path}` already exists")));
        }
        // The deepest existing ancestor must be inside the root, so creating
        // the missing directories cannot follow a link out of it.
        let mut ancestor = target.parent().map(Path::to_path_buf).unwrap_or_default();
        while tokio::fs::symlink_metadata(&ancestor).await.is_err() {
            match ancestor.parent() {
                Some(parent) => ancestor = parent.to_path_buf(),
                None => break,
            }
        }
        let resolved = tokio::fs::canonicalize(&ancestor)
            .await
            .map_err(|error| invalid(format!("`{path}`: {error}")))?;
        self.inside(&resolved, path)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                ToolError::Execution(format!("cannot create the directory: {error}"))
            })?;
        }
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
        Ok(())
    }

    /// Checks every edit, then writes every changed file. Returns the paths.
    pub async fn edit(&self, edits: Vec<Edit>) -> Result<Vec<String>, ToolError> {
        // path -> (resolved, new content), in first-seen order.
        let mut files: Vec<(String, PathBuf, String)> = Vec::new();
        for edit in edits {
            let index = match files.iter().position(|(path, ..)| *path == edit.path) {
                Some(index) => index,
                None => {
                    let resolved = self.existing_file(&edit.path).await?;
                    let bytes = tokio::fs::read(&resolved)
                        .await
                        .map_err(|error| invalid(format!("`{}`: {error}", edit.path)))?;
                    let text = String::from_utf8(bytes)
                        .map_err(|_| invalid(format!("`{}` is not UTF-8 text", edit.path)))?;
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
            let temporary = resolved.with_file_name(format!(
                ".{}.rah-{}",
                resolved
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file"),
                uuid::Uuid::new_v4().simple()
            ));
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
