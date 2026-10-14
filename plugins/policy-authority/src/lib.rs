//! `policy-authority`: the fail-closed tool policy (ADR 0036, section 5.1).
//!
//! It registers a `tool.before_invoke` handler that decides every tool call
//! from its own configuration only:
//!
//! - Each tool needs a host **assignment** in `tools` (exact name, or a
//!   pattern ending in `*` that matches by prefix; exact names win, then the
//!   longest prefix). A tool without an assignment is denied.
//! - The effective permission is the stricter of the assigned level and the
//!   level the tool declares. Tool metadata can only tighten a decision; it
//!   can never grant one.
//! - The effective level maps to Allow when it is listed in `allow`, to Ask
//!   when it is listed in `ask`, and to Deny otherwise.
//! - `decisions` fixes `allow`, `ask` or `deny` for a tool name or `prefix*`
//!   pattern (matched like `tools`). A matching decision replaces the rules
//!   above for that tool: the host can turn off one tool of a pattern, ask
//!   before a read, or allow a write without asking. It is host
//!   configuration; tool metadata still cannot grant anything.
//!
//! Configuration (all fields optional):
//!
//! ```toml
//! config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.read" = "read", "repo.*" = "read" }, decisions = { "repo.patch" = "deny" } }
//! ```
//!
//! The defaults are `allow = ["none", "read"]`, `ask = ["write", "execute"]`
//! and no assignments, which denies every tool. Without a mounted approver,
//! the `ToolInvoker` treats Ask as a denial.

use std::collections::BTreeMap;

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::{ToolBeforeInvoke, ToolDecision},
};
use rah_protocol::{PermissionLevel, ToolDefinition};
use serde::Deserialize;

/// Hook priority of the policy handler. Handlers with a higher priority run
/// first and can pre-empt the policy; mounting such a plugin is a trust
/// decision of the user.
pub const POLICY_PRIORITY: i32 = 0;

pub struct PolicyAuthorityPlugin {
    manifest: PluginManifest,
}

impl PolicyAuthorityPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")),
        }
    }
}

#[async_trait]
impl Plugin for PolicyAuthorityPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: PolicyConfig = if scope.config().is_null() {
            PolicyConfig::default()
        } else {
            scope.config_as()?
        };
        let policy = std::sync::Arc::new(Policy::new(config)?);
        scope.hook_bail::<ToolBeforeInvoke, _, _>(POLICY_PRIORITY, move |request| {
            let decision = policy.decide(&request.definition);
            async move { Some(decision) }
        });
        Ok(())
    }
}

/// The plugin configuration.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PolicyConfig {
    /// Effective levels that are allowed without confirmation.
    #[serde(default = "default_allow")]
    pub allow: Vec<PermissionLevel>,
    /// Effective levels that need confirmation through `tool.approval`.
    #[serde(default = "default_ask")]
    pub ask: Vec<PermissionLevel>,
    /// Host permission assignments by tool name or `prefix*` pattern.
    #[serde(default)]
    pub tools: BTreeMap<String, PermissionLevel>,
    /// Host decisions by tool name or `prefix*` pattern. A matching decision
    /// replaces the level rules for that tool.
    #[serde(default)]
    pub decisions: BTreeMap<String, Decision>,
}

/// A decision the host fixes for a tool.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Ask,
    Deny,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            allow: default_allow(),
            ask: default_ask(),
            tools: BTreeMap::new(),
            decisions: BTreeMap::new(),
        }
    }
}

fn default_allow() -> Vec<PermissionLevel> {
    vec![PermissionLevel::None, PermissionLevel::Read]
}

fn default_ask() -> Vec<PermissionLevel> {
    vec![PermissionLevel::Write, PermissionLevel::Execute]
}

/// Tool names and `prefix*` patterns: exact names win, then the longest
/// prefix.
#[derive(Clone, Debug)]
struct Patterns<T> {
    exact: BTreeMap<String, T>,
    /// Longest prefix first.
    prefixes: Vec<(String, T)>,
}

impl<T: Copy> Patterns<T> {
    fn new(entries: BTreeMap<String, T>, what: &str) -> Result<Self, String> {
        let mut exact = BTreeMap::new();
        let mut prefixes = Vec::new();
        for (pattern, value) in entries {
            match pattern.split_once('*') {
                None if pattern.is_empty() => {
                    return Err(format!("tool {what} names must not be empty"));
                }
                None => {
                    exact.insert(pattern, value);
                }
                Some((prefix, "")) => prefixes.push((prefix.to_owned(), value)),
                Some(_) => {
                    return Err(format!(
                        "tool pattern `{pattern}` may contain `*` only as its last character"
                    ));
                }
            }
        }
        prefixes.sort_by_key(|(prefix, _)| std::cmp::Reverse(prefix.len()));
        Ok(Self { exact, prefixes })
    }

    fn get(&self, name: &str) -> Option<T> {
        self.exact.get(name).copied().or_else(|| {
            self.prefixes
                .iter()
                .find(|(prefix, _)| name.starts_with(prefix.as_str()))
                .map(|(_, value)| *value)
        })
    }
}

/// A validated policy.
#[derive(Clone, Debug)]
pub struct Policy {
    allow: Vec<PermissionLevel>,
    ask: Vec<PermissionLevel>,
    assignments: Patterns<PermissionLevel>,
    decisions: Patterns<Decision>,
}

impl Policy {
    pub fn new(config: PolicyConfig) -> Result<Self, String> {
        if let Some(level) = config.allow.iter().find(|level| config.ask.contains(level)) {
            return Err(format!(
                "permission level `{}` is listed in both `allow` and `ask`",
                level_name(*level)
            ));
        }
        Ok(Self {
            allow: config.allow,
            ask: config.ask,
            assignments: Patterns::new(config.tools, "assignment")?,
            decisions: Patterns::new(config.decisions, "decision")?,
        })
    }

    /// The host assignment for a tool name, if any.
    pub fn assignment(&self, name: &str) -> Option<PermissionLevel> {
        self.assignments.get(name)
    }

    /// Decides one call. `definition` comes from the tool provider; only its
    /// name and a stricter declared permission are taken into account. A
    /// host decision for the tool replaces the level rules; it comes from
    /// the host's configuration, never from tool metadata.
    pub fn decide(&self, definition: &ToolDefinition) -> ToolDecision {
        let name = definition.name.as_str();
        match self.decisions.get(name) {
            Some(Decision::Allow) => return ToolDecision::Allow,
            Some(Decision::Ask) => {
                return ToolDecision::Ask {
                    reason: format!("the policy asks before tool `{name}` runs"),
                };
            }
            Some(Decision::Deny) => {
                return ToolDecision::Deny {
                    reason: format!("tool `{name}` is turned off in the policy"),
                };
            }
            None => {}
        }
        let Some(assigned) = self.assignment(name) else {
            return ToolDecision::Deny {
                reason: format!("tool `{name}` has no permission assignment"),
            };
        };
        let effective = stricter(assigned, definition.permission);
        let level = level_name(effective);
        if self.allow.contains(&effective) {
            ToolDecision::Allow
        } else if self.ask.contains(&effective) {
            ToolDecision::Ask {
                reason: format!("tool `{name}` requires `{level}` permission"),
            }
        } else {
            ToolDecision::Deny {
                reason: format!(
                    "tool `{name}` requires `{level}` permission, which is not granted"
                ),
            }
        }
    }
}

fn rank(level: PermissionLevel) -> u8 {
    match level {
        PermissionLevel::None => 0,
        PermissionLevel::Read => 1,
        PermissionLevel::Write => 2,
        PermissionLevel::Execute => 3,
    }
}

fn stricter(a: PermissionLevel, b: PermissionLevel) -> PermissionLevel {
    if rank(b) > rank(a) { b } else { a }
}

fn level_name(level: PermissionLevel) -> &'static str {
    match level {
        PermissionLevel::None => "none",
        PermissionLevel::Read => "read",
        PermissionLevel::Write => "write",
        PermissionLevel::Execute => "execute",
    }
}

impl PolicyAuthorityPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "allow": [
    "none",
    "read"
  ],
  "ask": [
    "write",
    "execute"
  ],
  "tools": {
    "fs.*": "read",
    "repo.*": "read",
    "shell.exec": "execute"
  },
  "decisions": {
    "web.*": "ask"
  }
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<PolicyConfig, _> =
        serde_json::from_str(PolicyAuthorityPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
