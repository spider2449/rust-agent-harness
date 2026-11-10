//! Enabling, reloading and disabling a saved plugin (ADR 0073).
//!
//! Everything here runs on the user's click in the "My plugins" view and
//! goes through the distribution's `admin` service: the plugin is mounted as
//! a `process` plugin under its name, the policy gets the assignments of
//! its tools that it lacks, and both are kept in the configuration file.
//! The model has no tool that reaches this module.

use std::path::Path;

use rah_api::{
    PluginId, PluginStatus,
    admin::{ADMIN, ConfigChange, MountRequest, PluginAdmin, PluginInstance},
    ui::UiError,
};
use serde_json::{Map, Value, json};

use crate::{Builder, manifest::Manifest, store::Saved};

/// The distribution's kind for a process plugin.
const PROCESS_KIND: &str = "process";
/// The distribution's kind for the tool policy.
const POLICY_KIND: &str = "policy-authority";

/// Where a saved plugin stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Saved; it does not run.
    Saved,
    /// It runs, and what runs is the last save.
    Enabled,
    /// It runs, but it was saved again since it started.
    Changed,
    /// Mounted by the user, then stopped in the Plugins panel.
    Stopped,
    /// Mounted, but not active; the text says why.
    NotRunning(String),
    /// Another plugin holds its name as an id.
    Taken,
}

impl State {
    pub(crate) fn label(&self) -> String {
        match self {
            Self::Saved => "not enabled".to_owned(),
            Self::Enabled => "enabled".to_owned(),
            Self::Changed => "enabled; reload to run the last save".to_owned(),
            Self::Stopped => "stopped".to_owned(),
            Self::NotRunning(why) => format!("enabled, not running ({why})"),
            Self::Taken => "its name is taken by another plugin".to_owned(),
        }
    }

    /// Whether the builder's plugin is mounted (or stopped) under its name.
    pub(crate) fn mounted(&self) -> bool {
        matches!(
            self,
            Self::Enabled | Self::Changed | Self::Stopped | Self::NotRunning(_)
        )
    }
}

fn describe(status: &PluginStatus) -> String {
    match status {
        PluginStatus::Pending { missing } => {
            let missing: Vec<&str> = missing.iter().map(|service| service.as_str()).collect();
            format!("waiting for {}", missing.join(", "))
        }
        PluginStatus::Failed { message } => message.clone(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Whether `instance` is the plugin saved in `dir`: a process plugin whose
/// entry file is in that directory.
fn ours(instance: &PluginInstance, dir: &Path) -> bool {
    instance.kind == PROCESS_KIND
        && instance
            .config
            .get("args")
            .and_then(|args| args.get(0))
            .and_then(Value::as_str)
            .is_some_and(|entry| Path::new(entry).parent() == Some(dir))
}

/// The policy assignments the tools of `manifest` need: name and level.
pub(crate) fn assignments(manifest: &Manifest) -> Vec<(String, String)> {
    manifest
        .tools
        .iter()
        .map(|tool| {
            let level = serde_json::to_value(tool.permission)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            (tool.name.as_str().to_owned(), level)
        })
        .collect()
}

/// A policy whose configuration an action changed.
struct PolicyChange {
    id: PluginId,
    before: Value,
    after: Value,
}

fn failed(error: impl std::fmt::Display) -> UiError {
    UiError::Failed(error.to_string())
}

impl Builder {
    fn admin(&self) -> Result<std::sync::Arc<dyn PluginAdmin>, UiError> {
        self.kernel.get(ADMIN).ok_or_else(|| {
            UiError::Failed(
                "this RAH has no `admin` service, so plugins are not mounted from here; \
                 follow the plugin's mount.toml"
                    .to_owned(),
            )
        })
    }

    /// Whether plugins can be enabled from the view.
    pub(crate) fn manages(&self) -> bool {
        self.kernel.get(ADMIN).is_some()
    }

    /// Where the saved plugin `name` stands.
    pub(crate) fn state(&self, name: &str) -> State {
        let id = PluginId::from(name);
        let status = self.kernel.status(&id);
        let Some(admin) = self.kernel.get(ADMIN) else {
            // Without the distribution's records, a plugin of this id is
            // someone's mount; the builder cannot tell whose.
            return match status {
                Some(_) => State::Taken,
                None => State::Saved,
            };
        };
        let Some(instance) = admin.instance(&id) else {
            return match status {
                Some(_) => State::Taken,
                None => State::Saved,
            };
        };
        if !ours(&instance, &self.store.plugin_dir(name)) {
            return State::Taken;
        }
        if instance.stopped {
            return State::Stopped;
        }
        match status {
            Some(PluginStatus::Active) if self.is_changed(name) => State::Changed,
            Some(PluginStatus::Active) => State::Enabled,
            Some(other) => State::NotRunning(describe(&other)),
            None => State::NotRunning("not mounted".to_owned()),
        }
    }

    fn is_changed(&self, name: &str) -> bool {
        self.changed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(name)
    }

    fn set_changed(&self, name: &str, changed: bool) {
        let mut names = self
            .changed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if changed {
            names.insert(name.to_owned());
        } else {
            names.remove(name);
        }
    }

    /// Notes that `name` was saved again: when it runs, what runs is now
    /// the earlier save. Returns whether it runs.
    pub(crate) fn saved_again(&self, name: &str) -> bool {
        let mounted = self.state(name).mounted();
        if mounted {
            self.set_changed(name, true);
        }
        mounted
    }

    /// The running policies with their configurations.
    fn policies(&self, admin: &dyn PluginAdmin) -> Vec<(PluginId, Map<String, Value>)> {
        self.kernel
            .plugins()
            .into_iter()
            .filter_map(|(id, _)| {
                let instance = admin.instance(&id)?;
                if instance.kind != POLICY_KIND || instance.stopped {
                    return None;
                }
                let config = match instance.config {
                    Value::Object(map) => map,
                    _ => Map::new(),
                };
                Some((id, config))
            })
            .collect()
    }

    /// Applies `edit` to the `tools` table of every running policy and
    /// restarts the ones it changed. When a policy does not come back, the
    /// ones changed so far are put back and the error says so.
    async fn edit_policies(
        &self,
        admin: &dyn PluginAdmin,
        edit: impl Fn(&mut Map<String, Value>) -> bool,
    ) -> Result<Vec<PolicyChange>, String> {
        let mut changes: Vec<PolicyChange> = Vec::new();
        for (id, config) in self.policies(admin) {
            let before = Value::Object(config.clone());
            let mut config = config;
            let tools = config.entry("tools").or_insert_with(|| json!({}));
            let Some(tools) = tools.as_object_mut() else {
                continue;
            };
            if !edit(tools) {
                continue;
            }
            let after = Value::Object(config);
            let outcome = match admin.configure(&id, after.clone()).await {
                Ok(PluginStatus::Active) => Ok(()),
                Ok(status) => Err(format!(
                    "the policy `{id}` did not restart ({})",
                    describe(&status)
                )),
                Err(error) => Err(format!("the policy `{id}` was not changed: {error}")),
            };
            changes.push(PolicyChange { id, before, after });
            if let Err(reason) = outcome {
                self.restore_policies(admin, &changes).await;
                return Err(reason);
            }
        }
        Ok(changes)
    }

    /// Puts the policies of `changes` back as they were, best effort.
    async fn restore_policies(&self, admin: &dyn PluginAdmin, changes: &[PolicyChange]) {
        for change in changes {
            let _ = admin.configure(&change.id, change.before.clone()).await;
        }
    }

    /// Asks the distribution to keep `changes`; the reasons it could not.
    async fn keep(&self, admin: &dyn PluginAdmin, changes: Vec<ConfigChange>) -> Option<String> {
        let mut reasons: Vec<String> = Vec::new();
        for change in changes {
            if let Err(error) = admin.persist(change).await {
                let reason = error.to_string();
                if !reasons.contains(&reason) {
                    reasons.push(reason);
                }
            }
        }
        (!reasons.is_empty()).then(|| reasons.join("; "))
    }

    fn policy_changes(changes: &[PolicyChange]) -> Vec<ConfigChange> {
        changes
            .iter()
            .map(|change| ConfigChange::Plugin {
                id: change.id.clone(),
                kind: POLICY_KIND.to_owned(),
                config: change.after.clone(),
            })
            .collect()
    }

    /// The configuration `saved` is mounted with. The working directory is
    /// the plugin directory, not the plugin's own: on Windows a process
    /// keeps its working directory open, and the plugin's directory must
    /// stay replaceable while it runs.
    fn mount_config(&self, saved: &Saved, program: &str) -> Value {
        let entry = saved.dir.join(saved.about.runtime.entry());
        let cwd = saved.dir.parent().unwrap_or(&saved.dir);
        json!({
            "program": program,
            "args": [entry.display().to_string()],
            "cwd": cwd.display().to_string(),
            "personal": true,
        })
    }

    /// Assigns the tools of `saved` in the policy, then mounts it. On a
    /// failure nothing of it stays: the plugin is unmounted and the
    /// assignments added for it are taken back.
    async fn start(
        &self,
        admin: &dyn PluginAdmin,
        saved: &Saved,
    ) -> Result<(Value, Vec<PolicyChange>), String> {
        let program = self.program(saved.about.runtime);
        if let Some(warning) = program.warning {
            return Err(format!(
                "{warning}; name the program in plugin-builder's configuration"
            ));
        }
        let wanted = assignments(&saved.manifest);
        let changes = self
            .edit_policies(admin, |tools| {
                let mut changed = false;
                for (name, level) in &wanted {
                    if !tools.contains_key(name) {
                        tools.insert(name.clone(), Value::from(level.as_str()));
                        changed = true;
                    }
                }
                changed
            })
            .await?;
        let config = self.mount_config(saved, &program.program);
        let request = MountRequest {
            kind: PROCESS_KIND.to_owned(),
            id: Some(saved.about.name.clone()),
            config: config.clone(),
            ..MountRequest::default()
        };
        let reason = match admin.mount(request).await {
            Ok((_, PluginStatus::Active)) => return Ok((config, changes)),
            Ok((id, status)) => {
                let _ = admin.unmount(&id).await;
                describe(&status)
            }
            Err(error) => error.to_string(),
        };
        self.restore_policies(admin, &changes).await;
        Err(reason)
    }

    /// Enables the saved plugin `name`, or with `reload` runs the last
    /// save of an enabled one. Returns one line for the user.
    pub(crate) async fn enable(&self, name: &str, reload: bool) -> Result<String, UiError> {
        let _one = self.lifecycle.lock().await;
        let saved = self
            .store
            .load(name)
            .await
            .map_err(|error| UiError::InvalidInput(format!("no saved plugin: {error}")))?;
        let admin = self.admin()?;
        let id = PluginId::from(name);
        match (self.state(name), reload) {
            (State::Taken, _) => {
                return Err(UiError::InvalidInput(format!(
                    "another plugin is mounted as `{name}`; unmount it in the Plugins panel first"
                )));
            }
            (State::Saved, true) => {
                return Err(UiError::InvalidInput(format!("`{name}` is not enabled")));
            }
            (state, false) if state.mounted() => {
                return Err(UiError::InvalidInput(format!(
                    "`{name}` is already enabled"
                )));
            }
            _ => {}
        }
        if reload {
            admin.unmount(&id).await.map_err(failed)?;
        }
        self.set_changed(name, false);
        let (config, changes) = match self.start(admin.as_ref(), &saved).await {
            Ok(started) => started,
            Err(reason) if reload => {
                // It no longer runs; the file must not start it again.
                let kept = self
                    .keep(admin.as_ref(), vec![ConfigChange::Removed { id }])
                    .await;
                self.announce();
                let mut message =
                    format!("`{name}` did not start again and is now disabled: {reason}.");
                if let Some(kept) = kept {
                    message.push_str(&format!(" Its entry is still in the file: {kept}."));
                }
                return Err(UiError::Failed(message));
            }
            Err(reason) => {
                return Err(UiError::Failed(format!(
                    "`{name}` was not enabled: {reason}"
                )));
            }
        };
        let assigned: Vec<String> = assignments(&saved.manifest)
            .into_iter()
            .map(|(tool, level)| format!("{tool} = {level}"))
            .collect();
        let mut to_keep = Self::policy_changes(&changes);
        to_keep.push(ConfigChange::Plugin {
            id,
            kind: PROCESS_KIND.to_owned(),
            config,
        });
        let kept = self.keep(admin.as_ref(), to_keep).await;
        self.announce();
        let mut message = format!("{} `{name}`.", if reload { "Reloaded" } else { "Enabled" });
        if !assigned.is_empty() {
            if self.policies(admin.as_ref()).is_empty() {
                message.push_str(" No policy plugin runs, so nothing was assigned.");
            } else {
                message.push_str(&format!(
                    " The policy assigns {}; an assignment it already had was kept.",
                    assigned.join(", ")
                ));
            }
        }
        if let Some(kept) = kept {
            message.push_str(&format!(" It runs until rah stops; not kept: {kept}."));
        }
        Ok(message)
    }

    /// Disables the plugin `name`: unmounts it, takes it out of the file
    /// and removes the assignments of its tools that still hold the level
    /// the plugin declares. Its files stay.
    pub(crate) async fn disable(&self, name: &str) -> Result<String, UiError> {
        let _one = self.lifecycle.lock().await;
        self.disable_locked(name).await
    }

    /// [`disable`](Self::disable) for a caller that holds the lifecycle
    /// lock.
    pub(crate) async fn disable_locked(&self, name: &str) -> Result<String, UiError> {
        let admin = self.admin()?;
        if !self.state(name).mounted() {
            return Err(UiError::InvalidInput(format!("`{name}` is not enabled")));
        }
        let id = PluginId::from(name);
        admin.unmount(&id).await.map_err(failed)?;
        self.set_changed(name, false);
        let declared = match self.store.load(name).await {
            Ok(saved) => assignments(&saved.manifest),
            Err(_) => Vec::new(),
        };
        let removed = self
            .edit_policies(admin.as_ref(), |tools| {
                let mut changed = false;
                for (tool, level) in &declared {
                    if tools.get(tool).and_then(Value::as_str) == Some(level.as_str()) {
                        tools.remove(tool);
                        changed = true;
                    }
                }
                changed
            })
            .await;
        let mut to_keep = vec![ConfigChange::Removed { id }];
        let mut message = format!("Disabled `{name}`; its files stay.");
        match removed {
            Ok(changes) => to_keep.extend(Self::policy_changes(&changes)),
            Err(reason) => message.push_str(&format!(
                " Its policy assignments were left ({reason}); they match no tool now."
            )),
        }
        if let Some(kept) = self.keep(admin.as_ref(), to_keep).await {
            message.push_str(&format!(
                " The file was not changed ({kept}); remove its entry by hand."
            ));
        }
        self.announce();
        Ok(message)
    }
}
