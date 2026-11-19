//! Human-operated skill directory management, shared by the CLI and Web UI.
use std::{
    io,
    path::{Path, PathBuf},
};

use rah_api::{async_trait, skills::SkillManager};
use std::io::Write;
use tokio::fs;

use crate::{parse, valid_name};

pub struct SkillDirectory {
    root: PathBuf,
    max_bytes: u64,
}

impl SkillDirectory {
    pub fn new(root: PathBuf, max_bytes: u64) -> Self {
        Self { root, max_bytes }
    }

    fn directory(&self, name: &str) -> io::Result<PathBuf> {
        if !valid_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "skill name must be 1-64 characters from a-z, 0-9, '-' and '_'",
            ));
        }
        Ok(self.root.join(name))
    }
}

fn located(path: &Path, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("`{}`: {error}", path.display()))
}

#[async_trait]
impl SkillManager for SkillDirectory {
    async fn preview(&self, repository: &str) -> io::Result<rah_api::skills::SkillPackage> {
        crate::packages::preview(repository, self.max_bytes).await
    }

    async fn packages(&self) -> io::Result<Vec<rah_api::skills::SkillPackage>> {
        crate::packages::list(self.root.clone()).await
    }

    async fn install(
        &self,
        selection: rah_api::skills::InstallPackage,
    ) -> io::Result<rah_api::skills::SkillPackage> {
        crate::packages::install(self.root.clone(), selection, self.max_bytes).await
    }

    async fn remove_package(&self, repository: &str) -> io::Result<()> {
        crate::packages::remove(&self.root, repository).await
    }

    async fn list(&self) -> io::Result<Vec<String>> {
        let mut entries = match fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(located(&self.root, e)),
        };
        let mut names = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if valid_name(&name)
                && check_directory(&entry.path()).await.is_ok()
                && fs::metadata(entry.path().join("SKILL.md"))
                    .await
                    .is_ok_and(|m| m.is_file())
            {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    async fn add(&self, name: &str, text: &str) -> io::Result<()> {
        let dir = self.directory(name)?;
        if text.len() as u64 > self.max_bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("skill exceeds max_skill_bytes ({})", self.max_bytes),
            ));
        }
        if parse(text).0.is_some_and(|declared| declared != name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("the file's declared skill name must match `{name}`"),
            ));
        }
        fs::create_dir_all(&self.root)
            .await
            .map_err(|e| located(&self.root, e))?;
        add(&dir, &dir.join("SKILL.md"), text).await
    }

    async fn delete(&self, name: &str) -> io::Result<()> {
        let dir = self.directory(name)?;
        delete(&dir, &dir.join("SKILL.md")).await
    }
}

async fn add(directory: &Path, path: &Path, text: &str) -> io::Result<()> {
    if let Err(error) = fs::create_dir(directory).await
        && error.kind() != io::ErrorKind::AlreadyExists
    {
        return Err(located(directory, error));
    }
    check_directory(directory).await?;
    // Finish publishing and cleaning up the staging file even if the HTTP
    // request disconnects while the filesystem operation is in progress.
    let path = path.to_owned();
    let text = text.to_owned();
    tokio::task::spawn_blocking(move || publish(&path, &text))
        .await
        .map_err(io::Error::other)?
}

fn publish(path: &Path, text: &str) -> io::Result<()> {
    let temp = path.with_file_name(format!(".skill-{}.tmp", std::process::id()));
    let mut out = std::fs::File::create_new(&temp).map_err(|e| located(&temp, e))?;
    let written = out.write_all(text.as_bytes());
    drop(out);
    let result = written.and_then(|()| std::fs::hard_link(&temp, path));
    let cleanup = std::fs::remove_file(&temp);
    result.map_err(|e| located(path, e))?;
    cleanup.map_err(|e| located(&temp, e))?;
    Ok(())
}

async fn delete(directory: &Path, path: &Path) -> io::Result<()> {
    check_directory(directory).await?;
    fs::remove_file(path).await.map_err(|e| located(path, e))?;
    match fs::remove_dir(directory).await {
        Ok(()) => {}
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::DirectoryNotEmpty | io::ErrorKind::NotFound
            ) => {}
        Err(e) => {
            return Err(io::Error::new(
                e.kind(),
                format!(
                    "deleted SKILL.md but could not remove {}",
                    located(directory, e)
                ),
            ));
        }
    }
    Ok(())
}

async fn check_directory(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)
        .await
        .map_err(|e| located(path, e))?;
    plain_directory(path, &meta)
}

pub(super) fn plain_directory(path: &Path, meta: &std::fs::Metadata) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("skill directory is a reparse point: `{}`", path.display()),
            ));
        }
    }
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("expected a skill directory at `{}`", path.display()),
        ));
    }
    Ok(())
}
