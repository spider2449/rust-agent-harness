use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use clap::Subcommand;
use rah_api::skills::SkillManager;
use rah_cli::{cli::GlobalArgs, resolve_data_paths};
use rah_plugin_skills_fs::{DEFAULT_MAX_SKILL_BYTES, SkillDirectory};

#[derive(Debug, Subcommand)]
pub enum SkillsCommand {
    /// Create an editable SKILL.md or import one from a UTF-8 file.
    Add {
        /// Skill directory name (1-64 lowercase letters, digits, '-' or '_').
        name: String,
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
    },
    /// Remove SKILL.md from the named directory, keeping any other files.
    Delete { name: String },
}

fn located(path: &Path, error: io::Error) -> String {
    format!("`{}`: {error}", path.display())
}

impl SkillsCommand {
    pub async fn run(&self, global: &GlobalArgs) -> Result<String, String> {
        let (Self::Add { name, .. } | Self::Delete { name }) = self;
        let prepared = global.prepare(false)?;
        let mut plugins = prepared
            .distribution
            .plugin
            .iter()
            .filter(|p| p.kind == "skills-fs");
        let mut config = plugins.next().map(|p| p.config.clone()).unwrap_or_default();
        if plugins.next().is_some() {
            return Err("multiple skills-fs plugins configured; select a configuration with one skills directory".into());
        }
        resolve_data_paths("skills-fs", &mut config, &prepared.home);
        let root = Path::new(
            config["dir"]
                .as_str()
                .ok_or("skills-fs.dir must be a directory path")?,
        );
        let path = root.join(name).join("SKILL.md");
        let limit = config["max_skill_bytes"]
            .as_u64()
            .unwrap_or(DEFAULT_MAX_SKILL_BYTES)
            .max(1);
        let skills = SkillDirectory::new(root.to_owned(), limit);
        match self {
            Self::Add { file, .. } => {
                let text = skill_text(name, file.as_deref(), limit)?;
                skills.add(name, &text).await.map_err(|e| e.to_string())?;
                Ok(format!("Added skill at `{}`", path.display()))
            }
            Self::Delete { .. } => {
                skills.delete(name).await.map_err(|e| e.to_string())?;
                Ok(format!("Deleted skill at `{}`", path.display()))
            }
        }
    }
}

fn skill_text(name: &str, file: Option<&Path>, limit: u64) -> Result<String, String> {
    let text = match file {
        Some(file) => {
            let mut text = String::new();
            fs::File::open(file)
                .and_then(|f| f.take(limit.saturating_add(1)).read_to_string(&mut text))
                .map_err(|e| located(file, e))?;
            text
        }
        None => format!("# {name}\n\nWrite the instructions for this skill here.\n"),
    };
    Ok(text)
}
