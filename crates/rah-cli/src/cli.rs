//! The options every `rah` command and `rah-desktop` share, and how they turn
//! a configuration file into the [`Distribution`] to run (ADR 0067 §4).
//! Command-line values override the file for one run and never write it.

use std::path::{Path, PathBuf};

use clap::Args;

use crate::{Distribution, Permissions, USER_CONFIG, default_data_home, select_distribution};

/// Options shared by every command.
#[derive(Clone, Debug, Default, Args)]
pub struct GlobalArgs {
    /// Model to run, as `<provider>/<model id>` or `<provider>` for its
    /// default model. Providers: anthropic, openai, deepseek,
    /// openai-compatible, llamacpp, mock.
    #[arg(long, global = true, value_name = "PROVIDER/ID")]
    pub model: Option<String>,
    /// What tools may do without asking: read-only, ask, auto-edit (writes
    /// run, programs ask) or full (every assigned tool runs without asking).
    #[arg(long, global = true, value_name = "MODE")]
    pub permissions: Option<String>,
    /// Workspace folder for file tools and programs. Default: the current
    /// directory.
    #[arg(long, global = true, value_name = "DIR")]
    pub root: Option<PathBuf>,
    /// Turn on an opt-in feature, such as `web-search` or `shell=git,cargo`;
    /// repeatable. `rah features` lists them.
    #[arg(long = "with", global = true, value_name = "FEATURE")]
    pub with: Vec<String>,
    /// Configuration file to use instead of `rah.toml` in the data directory.
    #[arg(long, global = true, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Data directory for sessions, storage, memory, skills and telemetry.
    /// Default: RAH_HOME, else `.rah` in the user's home directory.
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
}

/// Why a start that needs a model cannot go on, and the ways forward.
pub const NO_MODEL: &str = "no model is configured. Run `rah config init` to choose one and \
     write rah.toml, add `model = \"<provider>/<model id>\"` to the configuration file, or pass \
     --model, such as --model anthropic/claude-sonnet-5-5 (--model mock needs no key)";

/// Where a shown value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Flag,
    File,
    Default,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Self::Flag => "command line",
            Self::File => "file",
            Self::Default => "default",
        }
    }
}

/// A distribution ready to run, and where it came from.
#[derive(Debug)]
pub struct Prepared {
    pub distribution: Distribution,
    pub home: PathBuf,
    /// The configuration file that was read, if any.
    pub file: Option<PathBuf>,
    pub model: Origin,
    pub permissions: Origin,
    pub root: Origin,
}

/// `path` made absolute against the current directory, so a relative path
/// given on the command line means what it says where it was typed.
pub fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|error| format!("`{}`: {error}", path.display()))
}

impl GlobalArgs {
    /// `--data-dir`, else the default data home; `.rah` in the current
    /// directory only when no home directory is known.
    pub fn home(&self) -> Result<PathBuf, String> {
        let home = self
            .data_dir
            .clone()
            .or_else(default_data_home)
            .unwrap_or_else(|| PathBuf::from(".rah"));
        absolute(&home)
    }

    /// The configuration file `prepare` reads or `config init` writes:
    /// `--config`, else `rah.toml` in the data directory.
    pub fn config_path(&self) -> Result<PathBuf, String> {
        match &self.config {
            Some(path) => absolute(path),
            None => Ok(self.home()?.join(USER_CONFIG)),
        }
    }

    /// Whether nothing names a model yet: no `--config`, no `rah.toml` in
    /// the data directory and no `--model`. A start then needs
    /// `rah config init` first.
    pub fn unconfigured(&self) -> Result<bool, String> {
        Ok(self.config.is_none() && self.model.is_none() && !self.config_path()?.exists())
    }

    /// The distribution these options select, with the flags applied in
    /// order: features, root, permissions, model. Without `require_model`,
    /// a configuration that names no model is accepted (for commands that
    /// run none).
    pub fn prepare(&self, require_model: bool) -> Result<Prepared, String> {
        let home = self.home()?;
        let config = self.config.as_deref().map(absolute).transpose()?;
        let (mut distribution, file) = select_distribution(config.as_deref(), &home)?;
        let user = distribution.user.clone().unwrap_or_default();
        let from_file = |set: bool| if set { Origin::File } else { Origin::Default };
        let mut origins = (
            from_file(user.model.is_some() || file.is_some() && distribution.has_model()),
            from_file(user.permissions.is_some()),
            from_file(user.root.is_some()),
        );
        for request in &self.with {
            distribution.enable(request)?;
        }
        // Features first: the workspace plugins they mount (`format`, `lsp`)
        // then get the root too.
        if let Some(root) = &self.root {
            if !distribution.set_root(&absolute(root)?) {
                return Err("--root needs a `tools-builtin` or sandbox plugin".into());
            }
            origins.2 = Origin::Flag;
        } else if let Some(root) = distribution.user_root().map(Path::to_path_buf) {
            distribution.set_root(&root);
        }
        if let Some(permissions) = &self.permissions {
            let permissions = permissions
                .parse::<Permissions>()
                .map_err(|error| format!("--permissions: {error}"))?;
            distribution.set_permissions(permissions)?;
            origins.1 = Origin::Flag;
        }
        if let Some(model) = &self.model {
            distribution
                .set_model(model, &serde_json::Map::new())
                .map_err(|error| format!("--model: {error}"))?;
            origins.0 = Origin::Flag;
        }
        if require_model && distribution.user.is_some() && !distribution.has_model() {
            return Err(NO_MODEL.into());
        }
        Ok(Prepared {
            distribution,
            home,
            file,
            model: origins.0,
            permissions: origins.1,
            root: origins.2,
        })
    }
}
