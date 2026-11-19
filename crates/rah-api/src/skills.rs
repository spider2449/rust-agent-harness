//! Operator access to the mounted skills collection.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;

use crate::{ServiceKey, async_trait};

pub const SKILL_MANAGER_SERVICE: &str = "skill-manager";
pub const SKILL_MANAGER: ServiceKey<dyn SkillManager> = ServiceKey::new(SKILL_MANAGER_SERVICE);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPackage {
    /// Canonical public GitHub owner/repository.
    pub repository: String,
    pub revision: String,
    /// Directory names and their descriptions.
    pub skills: BTreeMap<String, String>,
    /// Include the Superpowers bootstrap instructions in model context.
    pub startup: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstallPackage {
    pub repository: String,
    pub revision: String,
    pub skills: Vec<String>,
    pub startup: bool,
}

/// Names identify installed entries, including entries with a different
/// display name. Add creates an entry exclusively. Delete removes its text.
#[async_trait]
pub trait SkillManager: Send + Sync {
    async fn list(&self) -> io::Result<Vec<String>>;
    async fn add(&self, name: &str, text: &str) -> io::Result<()>;
    async fn delete(&self, name: &str) -> io::Result<()>;
    async fn preview(&self, repository: &str) -> io::Result<SkillPackage>;
    async fn install(&self, selection: InstallPackage) -> io::Result<SkillPackage>;
    async fn packages(&self) -> io::Result<Vec<SkillPackage>>;
    async fn remove_package(&self, repository: &str) -> io::Result<()>;
}
