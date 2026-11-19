//! A package is one local ZIP, published only after all selected files and
//! its manifest are complete. Archive paths are keys and are never extracted.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Cursor, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use rah_api::skills::{InstallPackage, SkillPackage};
use serde::Deserialize;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{parse, valid_name};

const MAX_DOWNLOAD: usize = 16 * 1024 * 1024;
const MAX_FILES: usize = 8192;
const BOOTSTRAP: &str = "using-superpowers";
const SUPERPOWERS: &str = "obra/superpowers";
const MANIFEST: &str = "rah-package.json";

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

pub(crate) fn repository(input: &str) -> io::Result<String> {
    let input = input.trim().trim_end_matches('/');
    let slug = input.strip_prefix("https://github.com/").unwrap_or(input);
    let slug = slug.strip_suffix(".git").unwrap_or(slug);
    let (owner, repo) = slug
        .split_once('/')
        .ok_or_else(|| invalid("Enter a public GitHub repository URL."))?;
    if [owner, repo].iter().any(|s| {
        s.is_empty()
            || s.len() > 100
            || *s == "."
            || *s == ".."
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    }) {
        return Err(invalid("Expected a GitHub owner and repository name."));
    }
    Ok(slug.to_ascii_lowercase())
}

fn revision(sha: &str) -> io::Result<()> {
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Expected the previewed Git commit SHA."));
    }
    Ok(())
}

async fn fetch(url: String, limit: usize) -> io::Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent("RAH-skills")
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(io::Error::other)?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(io::Error::other)? {
        if chunk.len() > limit - bytes.len() {
            return Err(invalid("GitHub response exceeds the size limit."));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn archive(repo: &str, sha: &str) -> io::Result<Vec<u8>> {
    revision(sha)?;
    fetch(
        format!("https://codeload.github.com/{repo}/zip/{sha}"),
        MAX_DOWNLOAD,
    )
    .await
}

pub(crate) async fn preview(input: &str, limit: u64) -> io::Result<SkillPackage> {
    let repo = repository(input)?;
    #[derive(Deserialize)]
    struct Commit {
        sha: String,
    }
    let commit: Commit = serde_json::from_slice(
        &fetch(
            format!("https://api.github.com/repos/{repo}/commits/HEAD"),
            MAX_DOWNLOAD,
        )
        .await?,
    )?;
    let bytes = archive(&repo, &commit.sha).await?;
    tokio::task::spawn_blocking(move || {
        unpack(bytes, repo, commit.sha, limit, None).map(|(info, _)| info)
    })
    .await
    .map_err(io::Error::other)?
}

fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 1024
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
}

type Files = BTreeMap<String, Vec<u8>>;

fn unpack(
    bytes: Vec<u8>,
    repository: String,
    revision: String,
    limit: u64,
    selected: Option<&BTreeSet<String>>,
) -> io::Result<(SkillPackage, Files)> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    if zip.len() > MAX_FILES {
        return Err(invalid("Too many files in the repository."));
    }
    let mut skills = BTreeMap::new();
    let mut files = Files::new();
    let mut total = 0usize;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let Some((_, tail)) = file.name().split_once('/') else {
            continue;
        };
        let Some((name, relative)) = tail.strip_prefix("skills/").and_then(|s| s.split_once('/'))
        else {
            continue;
        };
        if file.is_dir() || !valid_name(name) || selected.is_some_and(|set| !set.contains(name)) {
            continue;
        }
        if selected.is_none() && relative != "SKILL.md" {
            continue;
        }
        if !safe_path(relative) || file.is_symlink() {
            return Err(invalid(
                "Skill contains an unsupported path or symbolic link.",
            ));
        }
        let (name, relative) = (name.to_owned(), relative.to_owned());
        let cap = if relative == "SKILL.md" {
            limit.min(MAX_DOWNLOAD as u64)
        } else {
            MAX_DOWNLOAD as u64
        };
        let cap = cap.min((MAX_DOWNLOAD - total) as u64);
        if file.size() > cap {
            return Err(invalid(format!(
                "Package size limit exceeded at {name}/{relative}"
            )));
        }
        let mut content = Vec::new();
        (&mut file).take(cap + 1).read_to_end(&mut content)?;
        if content.len() as u64 > cap {
            return Err(invalid("Package size limit exceeded."));
        }
        total += content.len();
        if relative == "SKILL.md" {
            let text =
                std::str::from_utf8(&content).map_err(|_| invalid("SKILL.md must be UTF-8"))?;
            let (_, description, body) = parse(text);
            let description = description
                .as_deref()
                .or_else(|| body.lines().find(|line| !line.trim().is_empty()))
                .unwrap_or("");
            skills.insert(name.clone(), description.chars().take(300).collect());
        }
        if files
            .insert(format!("{name}/{relative}"), content)
            .is_some()
        {
            return Err(invalid("Duplicate archive entry."));
        }
    }
    if skills.is_empty() || selected.is_some_and(|set| !set.iter().eq(skills.keys())) {
        return Err(invalid(
            "Expected a SKILL.md for each selected directory under skills/.",
        ));
    }
    let startup = repository == SUPERPOWERS && skills.contains_key(BOOTSTRAP);
    Ok((
        SkillPackage {
            repository,
            revision,
            skills,
            startup,
        },
        files,
    ))
}

fn store(root: &Path) -> PathBuf {
    root.join(".packages")
}
fn checked_store(root: &Path) -> io::Result<PathBuf> {
    let dir = store(root);
    crate::manage::plain_directory(&dir, &fs::symlink_metadata(&dir)?)?;
    Ok(dir)
}
fn path(root: &Path, repo: &str) -> io::Result<PathBuf> {
    Ok(store(root).join(format!("gh-{}.zip", repository(repo)?.replace('/', "@"))))
}

fn save(root: &Path, info: &SkillPackage, files: Files) -> io::Result<()> {
    let dest = path(root, &info.repository)?;
    fs::create_dir_all(store(root))?;
    let mut temp = tempfile::NamedTempFile::new_in(checked_store(root)?)?;
    let mut zip = ZipWriter::new(temp.as_file_mut());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file(MANIFEST, options)?;
    serde_json::to_writer(&mut zip, info)?;
    for (name, body) in files {
        zip.start_file(format!("skills/{name}"), options)?;
        zip.write_all(&body)?;
    }
    zip.finish()?;
    temp.persist_noclobber(dest).map_err(|e| e.error)?;
    Ok(())
}

pub(crate) async fn install(
    root: PathBuf,
    selection: InstallPackage,
    limit: u64,
) -> io::Result<SkillPackage> {
    let repo = repository(&selection.repository)?;
    let selected: BTreeSet<_> = selection.skills.into_iter().collect();
    if selected.is_empty() || !selected.iter().all(|n| valid_name(n)) {
        return Err(invalid("Select at least one valid skill."));
    }
    let bytes = archive(&repo, &selection.revision).await?;
    tokio::task::spawn_blocking(move || {
        let (mut info, files) = unpack(bytes, repo, selection.revision, limit, Some(&selected))?;
        if selection.startup && !info.startup {
            return Err(invalid(
                "Startup requires obra/superpowers and using-superpowers.",
            ));
        }
        info.startup = selection.startup;
        save(&root, &info, files)?;
        Ok(info)
    })
    .await
    .map_err(io::Error::other)?
}

pub(crate) async fn list(root: PathBuf) -> io::Result<Vec<SkillPackage>> {
    tokio::task::spawn_blocking(move || {
        let entries = match checked_store(&root).and_then(fs::read_dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut packages = Vec::new();
        for entry in entries {
            let entry = entry?;
            let p = entry.path();
            if !entry.file_type()?.is_file()
                || p.extension().and_then(|s| s.to_str()) != Some("zip")
            {
                continue;
            }
            let mut zip = match File::open(&p) {
                Ok(file) => ZipArchive::new(file)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            let info: SkillPackage =
                serde_json::from_reader(zip.by_name(MANIFEST)?.take(MAX_DOWNLOAD as u64))?;
            if path(&root, &info.repository)? == p {
                packages.push(info);
            }
        }
        packages.sort_by(|a, b| a.repository.cmp(&b.repository));
        Ok(packages)
    })
    .await
    .map_err(io::Error::other)?
}

pub(crate) async fn remove(root: &Path, repo: &str) -> io::Result<()> {
    let dir = store(root);
    crate::manage::plain_directory(&dir, &tokio::fs::symlink_metadata(&dir).await?)?;
    tokio::fs::remove_file(path(root, repo)?).await
}

pub(crate) async fn read(
    root: &Path,
    qualified: &str,
    relative: &str,
    limit: u64,
) -> io::Result<String> {
    let (repo, name) = qualified
        .split_once(':')
        .ok_or_else(|| invalid("Expected repository:skill."))?;
    let repo = if repo == "superpowers" {
        SUPERPOWERS
    } else {
        repo
    };
    if !valid_name(name) || !safe_path(relative) {
        return Err(invalid("Invalid skill asset path."));
    }
    let path = path(root, repo)?;
    let dir = store(root);
    crate::manage::plain_directory(&dir, &tokio::fs::symlink_metadata(&dir).await?)?;
    let key = format!("skills/{name}/{relative}");
    tokio::task::spawn_blocking(move || {
        if !fs::symlink_metadata(&path)?.is_file() {
            return Err(invalid("Expected a regular package archive."));
        }
        let mut zip = ZipArchive::new(File::open(path)?)?;
        let file = zip.by_name(&key)?;
        let limit = limit.min(MAX_DOWNLOAD as u64);
        if file.size() > limit {
            return Err(invalid("Skill asset exceeds the text limit."));
        }
        let mut text = String::new();
        file.take(limit + 1).read_to_string(&mut text)?;
        if text.len() as u64 > limit {
            return Err(invalid("Skill asset exceeds the text limit."));
        }
        Ok(text)
    })
    .await
    .map_err(io::Error::other)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Library;

    fn fixture(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (path, body) in files {
            zip.start_file(format!("repo-head/{path}"), SimpleFileOptions::default())
                .unwrap();
            zip.write_all(body).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[tokio::test]
    async fn selected_package_is_atomic_namespaced_and_supplies_bootstrap_and_assets() {
        let bytes = fixture(&[
            (
                "skills/using-superpowers/SKILL.md",
                b"---\ndescription: Bootstrap\n---\nStart here",
            ),
            ("skills/using-superpowers/refs/guide.md", b"companion"),
            ("skills/using-superpowers/image.png", &[0xff]),
            ("skills/other/SKILL.md", b"unselected"),
            ("hooks/session-start", b"ignored"),
        ]);
        let sha = "a".repeat(40);
        let (preview, _) =
            unpack(bytes.clone(), SUPERPOWERS.into(), sha.clone(), 256, None).unwrap();
        assert_eq!(preview.skills.len(), 2);
        let selected = BTreeSet::from([BOOTSTRAP.to_owned()]);
        let (info, files) = unpack(bytes, SUPERPOWERS.into(), sha, 256, Some(&selected)).unwrap();
        assert_eq!(files.len(), 3);
        let root = tempfile::tempdir().unwrap();
        save(root.path(), &info, files.clone()).unwrap();
        assert_eq!(
            save(root.path(), &info, files).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        let library = Library {
            dir: root.path().to_owned(),
            max_bytes: 256,
            max_skills: 64,
        };
        let scan = library.scan().await;
        assert_eq!(scan.skills[0].name, "obra/superpowers:using-superpowers");
        assert_eq!(scan.startup.as_deref(), Some("Start here"));
        let name = "superpowers:using-superpowers";
        assert_eq!(
            read(root.path(), name, "refs/guide.md", 256).await.unwrap(),
            "companion"
        );
        assert!(
            read(root.path(), name, "../other/SKILL.md", 256)
                .await
                .is_err()
        );
        assert!(read(root.path(), name, "image.png", 256).await.is_err());
        remove(root.path(), SUPERPOWERS).await.unwrap();
        let scan = library.scan().await;
        assert!(scan.startup.is_none() && scan.skills.is_empty());
    }

    #[test]
    fn package_inputs_reject_other_hosts_unsafe_paths_and_oversize_instructions() {
        assert_eq!(
            repository("https://github.com/Obra/Superpowers.git").unwrap(),
            SUPERPOWERS
        );
        for bad in ["https://other.example/a/b", "../name", "a/b/c", "a/b?tag=x"] {
            assert!(repository(bad).is_err());
        }
        let selected = BTreeSet::from(["one".into()]);
        for entries in [
            vec![("skills/one/SKILL.md", &b"oversize"[..])],
            vec![
                ("skills/one/SKILL.md", &b"ok"[..]),
                ("skills/one/../../file", &b"x"[..]),
            ],
        ] {
            assert!(
                unpack(
                    fixture(&entries),
                    "a/b".into(),
                    "a".repeat(40),
                    4,
                    Some(&selected)
                )
                .is_err()
            );
        }
    }
}
