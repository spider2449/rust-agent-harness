//! `skills-fs`: skills from a directory.
//!
//! A skill is `<dir>/<name>/SKILL.md`, optionally starting with front matter:
//!
//! ```text
//! ---
//! name: commit-message
//! description: Writes conventional commit messages.
//! ---
//! (instructions)
//! ```
//!
//! Without front matter the name is the directory name and the description
//! is the first non-empty line. Every model request gets one system item
//! listing the skills (name and description); `skills.load {name}` (`read`)
//! returns a skill's full text, so only skills in use take context. The
//! directory is read on every request, so edits apply immediately. Skills
//! are text only: they grant no permission and define no executable tools.
//!
//! Configuration: `{ "dir": ".rah/skills", "max_skills": 64,
//! "max_skill_bytes": 65536 }`.

use std::{path::PathBuf, sync::Arc};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::ModelBeforeRequest,
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{
    ContextItem, PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput,
};
use serde::Deserialize;
use serde_json::{Value, json};

const MAX_DESCRIPTION_CHARS: usize = 300;

pub struct SkillsPlugin {
    manifest: PluginManifest,
}

impl SkillsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides([TOOLS_SERVICE]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    dir: Option<PathBuf>,
    max_skills: Option<usize>,
    max_skill_bytes: Option<u64>,
}

#[async_trait]
impl Plugin for SkillsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let library = Arc::new(Library {
            dir: config.dir.unwrap_or_else(|| PathBuf::from(".rah/skills")),
            max_skills: config.max_skills.unwrap_or(64).max(1),
            max_bytes: config.max_skill_bytes.unwrap_or(64 * 1024).max(1),
        });
        let lister = library.clone();
        scope.hook_waterfall::<ModelBeforeRequest, _, _>(0, move |mut request| {
            let lister = lister.clone();
            async move {
                let skills = lister.scan().await;
                if !skills.is_empty() {
                    let at = request
                        .items
                        .iter()
                        .position(|item| !matches!(item, ContextItem::System { .. }))
                        .unwrap_or(request.items.len());
                    request.items.insert(
                        at,
                        ContextItem::System {
                            text: listing(&skills),
                        },
                    );
                }
                request
            }
        });
        provide_tools(scope, vec![Arc::new(LoadSkill(library))])?;
        Ok(())
    }
}

/// One skill's summary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
}

/// Skill names: 1-64 of `[a-z0-9_-]`.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Splits optional front matter from the body: `(name, description, body)`.
pub fn parse(text: &str) -> (Option<String>, Option<String>, &str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let opening = lines.next().map(str::trim);
    if opening != Some("---") {
        return (None, None, text);
    }
    let (mut name, mut description) = (None, None);
    let mut consumed = text.split_inclusive('\n').next().map_or(0, str::len);
    for line in lines {
        consumed += line.len();
        let trimmed = line.trim();
        if trimmed == "---" {
            return (name, description, &text[consumed..]);
        }
        if let Some((key, value)) = trimmed.split_once(':') {
            let value = value.trim().trim_matches('"').to_owned();
            match key.trim() {
                "name" => name = Some(value),
                "description" => description = Some(value),
                _ => {}
            }
        }
    }
    // No closing marker: treat everything as body.
    (None, None, text)
}

fn listing(skills: &[Skill]) -> String {
    let mut text = String::from(
        "Skills are available. Call skills.load with a skill's name to read its full \
         instructions before following them:\n",
    );
    for skill in skills {
        text.push_str(&format!("- {}: {}\n", skill.name, skill.description));
    }
    text
}

struct Library {
    dir: PathBuf,
    max_skills: usize,
    max_bytes: u64,
}

impl Library {
    /// The text of `<dir>/<directory>/SKILL.md`, if it is a bounded regular
    /// file.
    async fn read(&self, directory: &str) -> Option<String> {
        let path = self.dir.join(directory).join("SKILL.md");
        let metadata = tokio::fs::metadata(&path).await.ok()?;
        if !metadata.is_file() || metadata.len() > self.max_bytes {
            return None;
        }
        String::from_utf8(tokio::fs::read(&path).await.ok()?).ok()
    }

    /// Skills sorted by name; unreadable or invalid ones are skipped.
    async fn scan(&self) -> Vec<Skill> {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return Vec::new();
        };
        let mut skills = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Some(directory) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !valid_name(&directory) {
                continue;
            }
            let Some(text) = self.read(&directory).await else {
                continue;
            };
            let (name, description, body) = parse(&text);
            let name = name.filter(|name| valid_name(name)).unwrap_or(directory);
            let description = description
                .or_else(|| {
                    body.lines()
                        .map(str::trim)
                        .find(|l| !l.is_empty())
                        .map(str::to_owned)
                })
                .unwrap_or_default()
                .trim_start_matches('#')
                .trim()
                .chars()
                .take(MAX_DESCRIPTION_CHARS)
                .collect();
            if !skills.iter().any(|skill: &Skill| skill.name == name) {
                skills.push(Skill { name, description });
            }
        }
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills.truncate(self.max_skills);
        skills
    }

    /// The body of the skill called `name` (by front matter or directory).
    async fn load(&self, name: &str) -> Option<String> {
        if !valid_name(name) {
            return None;
        }
        if let Some(text) = self.read(name).await {
            let (declared, _, body) = parse(&text);
            if declared.is_none_or(|declared| declared == name) {
                return Some(body.trim().to_owned());
            }
        }
        // A skill whose front matter names it differently from its directory.
        let mut entries = tokio::fs::read_dir(&self.dir).await.ok()?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let Some(directory) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !valid_name(&directory) {
                continue;
            }
            if let Some(text) = self.read(&directory).await {
                let (declared, _, body) = parse(&text);
                if declared.as_deref() == Some(name) {
                    return Some(body.trim().to_owned());
                }
            }
        }
        None
    }
}

struct LoadSkill(Arc<Library>);

#[async_trait]
impl Tool for LoadSkill {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("skills.load"),
            description: "Returns the full instructions of a listed skill.".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "name": { "type": "string", "minLength": 1, "maxLength": 64 } },
                "required": ["name"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let name = input
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::InvalidInput("expected a string `name`".into()))?;
        let body = self
            .0
            .load(name)
            .await
            .ok_or_else(|| ToolError::InvalidInput(format!("there is no skill `{name}`")))?;
        Ok(ToolOutput {
            content: vec![ToolContent::Text(body)],
            is_error: false,
        })
    }
}

impl SkillsPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "dir": ".rah/skills",
  "max_skills": 64,
  "max_skill_bytes": 65536
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(SkillsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
