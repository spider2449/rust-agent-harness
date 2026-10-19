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
//! - `commands` lets the host run chosen `shell.exec` commands without
//!   asking (ADR 0050). A rule is a program and its arguments, matched
//!   against the call's `program` and `args` as exact strings; a last element
//!   `"*"` also accepts any further arguments. It only turns an Ask into an
//!   Allow: it never overrides a `decisions` entry or a denial, and a call
//!   that matches no rule is judged as before.
//!
//! - `approve` lets the host run chosen tools without asking (ADR 0054). It
//!   lists exact tool names, no patterns. Like `commands`, it only turns an
//!   Ask into an Allow: a `decisions` entry, a missing assignment or a denied
//!   level still wins.
//!
//! Configuration (all fields optional):
//!
//! ```toml
//! config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.read" = "read", "repo.*" = "read" }, decisions = { "repo.patch" = "deny" }, commands = [["git", "status"], ["cargo", "check", "*"]], approve = ["repo.create-file"] }
//! ```
//!
//! The defaults are `allow = ["none", "read"]`, `ask = ["write", "execute"]`
//! and no assignments, which denies every tool. Without a mounted approver,
//! the `ToolInvoker` treats Ask as a denial.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicUsize, Ordering},
};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::{ToolBeforeInvoke, ToolDecision, ToolRequestContext},
    tools::{TOOL_ADMISSION, TOOL_ADMISSION_SERVICE, ToolAdmission, ToolPermit},
};
use rah_protocol::{PermissionLevel, ToolDefinition};
use serde::Deserialize;

/// Hook priority of the policy handler. Handlers with a higher priority run
/// first and can pre-empt the policy; mounting such a plugin is a trust
/// decision of the user.
pub const POLICY_PRIORITY: i32 = 0;

/// How many tool calls may run at once without `max_in_flight` (the limit
/// of the legacy host).
pub const DEFAULT_MAX_IN_FLIGHT: usize = 128;

pub struct PolicyAuthorityPlugin {
    manifest: PluginManifest,
}

impl PolicyAuthorityPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([TOOL_ADMISSION_SERVICE]),
        }
    }
}

#[async_trait]
impl Plugin for PolicyAuthorityPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        Policy::new(rah_api::config::parse(config)?)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: PolicyConfig = rah_api::config::parse(scope.config())?;
        let in_flight = InFlight::new(config.max_in_flight.unwrap_or(DEFAULT_MAX_IN_FLIGHT));
        let policy = std::sync::Arc::new(Policy::new(config)?);
        scope.hook_bail::<ToolBeforeInvoke, _, _>(POLICY_PRIORITY, move |request| {
            let decision = policy.decide_call(&request);
            async move { Some(decision) }
        });
        scope.provide(TOOL_ADMISSION, std::sync::Arc::new(in_flight))?;
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
    /// `shell.exec` commands that run without asking (ADR 0050): a program,
    /// its arguments, and optionally a last element `"*"` for any further
    /// arguments.
    #[serde(default)]
    pub commands: Vec<Vec<String>>,
    /// Exact tool names that run without asking (ADR 0054).
    #[serde(default)]
    pub approve: Vec<String>,
    /// The most tool calls that may run at once, across all sessions
    /// (default [`DEFAULT_MAX_IN_FLIGHT`]). A call over the limit is not
    /// started.
    #[serde(default)]
    pub max_in_flight: Option<usize>,
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
            commands: Vec::new(),
            approve: Vec::new(),
            max_in_flight: None,
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

/// The tool whose commands `commands` rules apply to.
const COMMAND_TOOL: &str = "shell.exec";

/// One `commands` rule: a program and its arguments, optionally followed by
/// any further arguments.
#[derive(Clone, Debug)]
struct CommandRule {
    program: String,
    args: Vec<String>,
    /// Whether arguments beyond `args` are accepted.
    any_more: bool,
}

impl CommandRule {
    fn new(mut words: Vec<String>) -> Result<Self, String> {
        let any_more = words.len() > 1 && words.last().is_some_and(|word| word == "*");
        if any_more {
            words.pop();
        }
        if words.is_empty() || words[0].trim().is_empty() {
            return Err("a `commands` rule needs a program".into());
        }
        if words.iter().any(|word| word == "*") {
            return Err("in a `commands` rule `*` may only be the last element".into());
        }
        let program = words.remove(0);
        Ok(Self {
            program,
            args: words,
            any_more,
        })
    }

    fn matches(&self, program: &str, args: &[&str]) -> bool {
        program == self.program
            && if self.any_more {
                args.len() >= self.args.len()
            } else {
                args.len() == self.args.len()
            }
            && self.args.iter().zip(args).all(|(rule, arg)| rule == arg)
    }
}

/// A validated policy.
#[derive(Clone, Debug)]
pub struct Policy {
    allow: Vec<PermissionLevel>,
    ask: Vec<PermissionLevel>,
    assignments: Patterns<PermissionLevel>,
    decisions: Patterns<Decision>,
    commands: Vec<CommandRule>,
    approve: BTreeSet<String>,
}

impl Policy {
    pub fn new(config: PolicyConfig) -> Result<Self, String> {
        if config.max_in_flight == Some(0) {
            return Err("`max_in_flight` must be at least 1".into());
        }
        if let Some(level) = config.allow.iter().find(|level| config.ask.contains(level)) {
            return Err(format!(
                "permission level `{}` is listed in both `allow` and `ask`",
                level_name(*level)
            ));
        }
        if let Some(name) = config
            .approve
            .iter()
            .find(|name| name.trim().is_empty() || name.contains('*'))
        {
            return Err(format!(
                "`approve` takes exact tool names, not `{name}`: a name must not be empty or contain `*`"
            ));
        }
        Ok(Self {
            allow: config.allow,
            ask: config.ask,
            assignments: Patterns::new(config.tools, "assignment")?,
            decisions: Patterns::new(config.decisions, "decision")?,
            commands: config
                .commands
                .into_iter()
                .map(CommandRule::new)
                .collect::<Result<_, _>>()?,
            approve: config.approve.into_iter().collect(),
        })
    }

    /// The host assignment for a tool name, if any.
    pub fn assignment(&self, name: &str) -> Option<PermissionLevel> {
        self.assignments.get(name)
    }

    /// Decides one call: [`Policy::decide`] for its tool, except that an
    /// `approve` entry for the tool, or a `commands` rule for `shell.exec`,
    /// turns an Ask into an Allow. The call input is untrusted: anything but
    /// a program string and an array of argument strings matches no rule.
    pub fn decide_call(&self, request: &ToolRequestContext) -> ToolDecision {
        let decision = self.decide(&request.definition);
        let name = request.definition.name.as_str();
        if !matches!(decision, ToolDecision::Ask { .. }) || self.decisions.get(name).is_some() {
            return decision;
        }
        if self.approve.contains(name) {
            return ToolDecision::Allow;
        }
        if name != COMMAND_TOOL {
            return decision;
        }
        let input = &request.call.input.0;
        let Some(program) = input.get("program").and_then(serde_json::Value::as_str) else {
            return decision;
        };
        let Some(args) = input.get("args").map_or(Some(Vec::new()), |args| {
            args.as_array()?
                .iter()
                .map(serde_json::Value::as_str)
                .collect::<Option<Vec<_>>>()
        }) else {
            return decision;
        };
        if self
            .commands
            .iter()
            .any(|rule| rule.matches(program, &args))
        {
            ToolDecision::Allow
        } else {
            decision
        }
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
  },
  "commands": [
    ["git", "status"]
  ],
  "max_in_flight": 128
}"#;
}

/// Counts running tool calls and refuses one over the limit. The
/// `ToolInvoker` holds the permit while the tool runs, so the count is
/// released however the call ends.
pub struct InFlight {
    max: usize,
    running: std::sync::Arc<AtomicUsize>,
}

impl InFlight {
    pub fn new(max: usize) -> Self {
        Self {
            max,
            running: std::sync::Arc::default(),
        }
    }

    /// How many admitted calls are running.
    pub fn running(&self) -> usize {
        self.running.load(Ordering::SeqCst)
    }
}

struct Running(std::sync::Arc<AtomicUsize>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl ToolAdmission for InFlight {
    fn admit(&self, _request: &ToolRequestContext) -> Result<ToolPermit, String> {
        let mut running = self.running.load(Ordering::SeqCst);
        loop {
            if running >= self.max {
                return Err(format!(
                    "{} tool calls are already running, the most the policy allows \
                     (`max_in_flight`)",
                    self.max
                ));
            }
            match self.running.compare_exchange_weak(
                running,
                running + 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(current) => running = current,
            }
        }
        Ok(ToolPermit::new(Running(self.running.clone())))
    }
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
