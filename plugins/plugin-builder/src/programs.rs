//! Finding the interpreter `mount.toml` starts a plugin with.
//!
//! The process host starts `program` by name through `PATH`, so a name that
//! is not on `PATH` fails only when the user mounts the plugin. At save time
//! the builder looks for the program instead and writes its full path. It
//! only checks that files exist; it never runs anything.

use std::path::{Path, PathBuf};

use crate::store::Runtime;

/// Where to look for programs, read from the environment once at start.
#[derive(Clone, Debug, Default)]
pub struct Search {
    /// The directories of `PATH`, in order.
    pub path: Vec<PathBuf>,
    /// Executable extensions tried after the bare name (Windows `PATHEXT`).
    pub extensions: Vec<String>,
    /// Directories holding uv-managed Python installs, one per version.
    pub uv_python: Vec<PathBuf>,
}

impl Search {
    /// The search of this process: `PATH`, `PATHEXT` and uv's Python
    /// directory (`UV_PYTHON_INSTALL_DIR`, else uv's default).
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        let path = var("PATH")
            .map(|value| std::env::split_paths(&value).collect())
            .unwrap_or_default();
        let extensions = if cfg!(windows) {
            var("PATHEXT")
                .map(|value| {
                    value
                        .to_string_lossy()
                        .split(';')
                        .filter(|ext| !ext.is_empty())
                        .map(str::to_ascii_lowercase)
                        .collect()
                })
                .unwrap_or_else(|| vec![".exe".to_owned(), ".cmd".to_owned(), ".bat".to_owned()])
        } else {
            Vec::new()
        };
        let uv_python = match var("UV_PYTHON_INSTALL_DIR") {
            Some(dir) => vec![PathBuf::from(dir)],
            None if cfg!(windows) => var("APPDATA")
                .map(|dir| vec![PathBuf::from(dir).join("uv").join("python")])
                .unwrap_or_default(),
            None => var("HOME")
                .map(|dir| vec![PathBuf::from(dir).join(".local/share/uv/python")])
                .unwrap_or_default(),
        };
        Self {
            path,
            extensions,
            uv_python,
        }
    }
}

/// The program for `runtime`: its full path when found, and a warning for
/// the user when not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub program: String,
    pub warning: Option<String>,
}

/// Finds `program` (a name or a path) for `runtime`. A Python that is not
/// on `PATH` is looked for among uv's installs only with `uv_fallback`, when
/// the user did not name one.
pub fn find(program: &str, runtime: Runtime, search: &Search, uv_fallback: bool) -> Found {
    let as_path = Path::new(program);
    if as_path.is_absolute() || program.contains(['/', '\\']) {
        return if is_file(as_path) {
            Found {
                program: program.to_owned(),
                warning: None,
            }
        } else {
            Found {
                program: program.to_owned(),
                warning: Some(format!(
                    "`{program}` does not exist; set `program` to the interpreter's full path"
                )),
            }
        };
    }
    if let Some(found) = on_path(program, search) {
        return Found {
            program: found.display().to_string(),
            warning: None,
        };
    }
    let uv = runtime == Runtime::Python && uv_fallback;
    if uv && let Some(found) = uv_python(search) {
        return Found {
            program: found.display().to_string(),
            warning: None,
        };
    }
    Found {
        program: program.to_owned(),
        warning: Some(format!(
            "`{program}` was not found on PATH{}; set `program` to the interpreter's full path, \
             or `{}` in the plugin-builder configuration",
            if uv {
                " or among uv's Python installs"
            } else {
                ""
            },
            runtime.label()
        )),
    }
}

fn is_file(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// `name` in a `PATH` directory, also with each extension. The Windows
/// Store's `WindowsApps` aliases are skipped: they open the Store instead of
/// running an interpreter.
fn on_path(name: &str, search: &Search) -> Option<PathBuf> {
    let names: Vec<String> = std::iter::once(name.to_owned())
        .chain(search.extensions.iter().map(|ext| format!("{name}{ext}")))
        .collect();
    search
        .path
        .iter()
        .filter(|dir| {
            !dir.components()
                .any(|part| part.as_os_str().eq_ignore_ascii_case("WindowsApps"))
        })
        .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
        .find(|candidate| is_file(candidate))
}

/// The newest uv-managed CPython 3 (`cpython-3.<minor>...` directories).
fn uv_python(search: &Search) -> Option<PathBuf> {
    let executable = |dir: &Path| {
        if cfg!(windows) {
            dir.join("python.exe")
        } else {
            dir.join("bin").join("python3")
        }
    };
    let mut best: Option<(u32, PathBuf)> = None;
    for root in &search.uv_python {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(minor) = name
                .strip_prefix("cpython-3.")
                .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|minor| minor.parse::<u32>().ok())
            else {
                continue;
            };
            let program = executable(&entry.path());
            if is_file(&program) && best.as_ref().is_none_or(|(seen, _)| minor > *seen) {
                best = Some((minor, program));
            }
        }
    }
    best.map(|(_, program)| program)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "rah-plugin-builder-programs-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, path: &str) -> PathBuf {
            let full = self.0.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, "").unwrap();
            full
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_program_on_path_is_written_with_its_full_path() {
        let dir = Dir::new("path");
        let store = dir.file("WindowsApps/python.exe");
        let real = dir.file("bin/python.exe");
        let search = Search {
            path: vec![
                store.parent().unwrap().into(),
                real.parent().unwrap().into(),
            ],
            extensions: vec![".exe".into()],
            uv_python: Vec::new(),
        };
        // The Store alias comes first on PATH and is skipped.
        let found = find("python", Runtime::Python, &search, true);
        assert_eq!(PathBuf::from(&found.program), real);
        assert_eq!(found.warning, None);
    }

    #[test]
    fn python_falls_back_to_the_newest_uv_install() {
        let dir = Dir::new("uv");
        let executable = if cfg!(windows) {
            "python.exe"
        } else {
            "bin/python3"
        };
        dir.file(&format!("uv/cpython-3.9-x86_64/{executable}"));
        let newest = dir.file(&format!("uv/cpython-3.14.6-x86_64/{executable}"));
        dir.file("uv/cpython-3.15-x86_64/README");
        dir.file("uv/pypy-3.20/x");
        let search = Search {
            path: Vec::new(),
            extensions: Vec::new(),
            uv_python: vec![dir.0.join("uv")],
        };
        let found = find("python", Runtime::Python, &search, true);
        assert_eq!(PathBuf::from(&found.program), newest);
        assert_eq!(found.warning, None);
        // Not for a Python the user named, and not for Node.
        let named = find("py3", Runtime::Python, &search, false);
        assert_eq!(named.program, "py3");
        assert!(!named.warning.unwrap().contains("uv"));
        let node = find("node", Runtime::Node, &search, true);
        assert_eq!(node.program, "node");
        assert!(node.warning.unwrap().contains("not found on PATH"));
    }

    #[test]
    fn a_missing_program_is_kept_with_a_warning() {
        let search = Search::default();
        let found = find("python", Runtime::Python, &search, true);
        assert_eq!(found.program, "python");
        let warning = found.warning.unwrap();
        assert!(
            warning.contains("not found on PATH or among uv's Python installs")
                && warning.contains("`python` in the plugin-builder"),
            "{warning}"
        );

        let dir = Dir::new("given");
        let given = dir.file("py/python.exe").display().to_string();
        assert_eq!(
            find(&given, Runtime::Python, &search, false),
            Found {
                program: given.clone(),
                warning: None
            }
        );
        let missing = dir.0.join("nope/python.exe").display().to_string();
        let found = find(&missing, Runtime::Python, &search, false);
        assert_eq!(found.program, missing);
        assert!(found.warning.unwrap().contains("does not exist"));
    }
}
