//! The "My plugins" view: where the user manages the saved plugins
//! (ADR 0062, ADR 0073).
//!
//! The list shows every saved plugin with its state. A plugin's details hold
//! the actions on it: enable, reload, disable and delete. Separate pages hold
//! backups, export and import. Every action runs
//! only on the user's click; the model cannot reach a view.

use std::{
    collections::BTreeSet,
    future::Future,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use rah_api::{
    async_trait,
    ui::{
        ActionOutcome, Field, FieldKind, KeyValue, Node, UiError, UiViews, View, ViewContext,
        ViewSpec,
    },
};
use serde_json::Value;

use crate::{
    BACKUP_ACTION, Builder, CONFIRM_FIELD, DELETE_PREFIX, DETAILS_PREFIX, DISABLE_PREFIX,
    ENABLE_PREFIX, EXPORT_ACTION, EXPORT_PREFIX, IMPORT_ACTION, LIST_ACTION, READ_FIELD,
    RELOAD_PREFIX, RESTORE_ACTION, VIEW,
    lifecycle::{State, assignments},
    manifest, plan,
    store::{BACKUP_DIR, MOUNT_FILE, SaveError, Saved, Store},
};

/// The restore form's field that names the backup.
const BACKUP_FIELD: &str = "backup";
const SHOW_IMPORT: &str = "show-import";
const SHOW_TRANSFER: &str = "show-transfer";

#[derive(Clone)]
enum Page {
    Library,
    Plugin(String),
    Import,
    Transfer,
}

fn back_to_list() -> Node {
    Node::Button {
        action: LIST_ACTION.to_owned(),
        label: "Back to the list".to_owned(),
    }
}

/// A compact library with selected-plugin details and utility pages.
pub(crate) struct BuilderViews {
    builder: Arc<Builder>,
    /// The library, a selected plugin, or a utility page.
    shown: Mutex<Page>,
}

fn done(message: String) -> ActionOutcome {
    ActionOutcome {
        message: Some(message),
        rerender: true,
    }
}

fn boolean(name: &str, label: impl Into<String>) -> Field {
    Field {
        name: name.to_owned(),
        label: label.into(),
        kind: FieldKind::Boolean,
        value: String::new(),
        options: Vec::new(),
    }
}

fn text_field(name: &str, label: impl Into<String>) -> Field {
    Field {
        name: name.to_owned(),
        label: label.into(),
        kind: FieldKind::Text,
        value: String::new(),
        options: Vec::new(),
    }
}

fn or_none(items: Vec<String>) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

#[async_trait]
impl UiViews for BuilderViews {
    fn views(&self) -> Vec<ViewSpec> {
        vec![ViewSpec::new(VIEW, "My plugins")]
    }

    async fn render(&self, view: &str, _: &ViewContext) -> Result<View, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.to_owned()));
        }
        let builder = &self.builder;
        let store = &builder.store;
        let saved = store.list().await;
        let page = self.shown();
        let mut nodes = vec![Node::Text {
            text: match &page {
                Page::Library => "Select a plugin to view its settings and permissions.",
                Page::Plugin(_) => "Review this plugin before enabling or reloading it.",
                Page::Import => "Import a bundle. Imported plugins stay disabled.",
                Page::Transfer => "Back up or move your saved plugins.",
            }
            .to_owned(),
        }];
        if let Page::Plugin(name) = &page
            && let Some(plugin) = saved.iter().find(|plugin| plugin.about.name == *name)
        {
            nodes.extend(self.details(plugin).await);
            return Ok(View { nodes });
        }
        if matches!(page, Page::Import) {
            nodes.push(back_to_list());
            nodes.extend(import_form());
            return Ok(View { nodes });
        }
        if !matches!(page, Page::Transfer) {
            if saved.is_empty() {
                nodes.push(Node::Text {
                    text: "No personal plugins yet.".to_owned(),
                });
            } else {
                nodes.push(Node::Table {
                    columns: ["Plugin", "Status"].map(str::to_owned).to_vec(),
                    rows: saved
                        .iter()
                        .map(|plugin| {
                            vec![
                                plugin.about.name.clone(),
                                builder.state(&plugin.about.name).label(),
                            ]
                        })
                        .collect(),
                    row_actions: saved
                        .iter()
                        .map(|plugin| Some(format!("{DETAILS_PREFIX}{}", plugin.about.name)))
                        .collect(),
                });
            }
            nodes.push(Node::Button {
                action: SHOW_IMPORT.to_owned(),
                label: "Import".to_owned(),
            });
            nodes.push(Node::Button {
                action: SHOW_TRANSFER.to_owned(),
                label: "Backups & export".to_owned(),
            });
            return Ok(View { nodes });
        }
        nodes.push(back_to_list());
        let backups = store.backups().await;
        if saved.is_empty() {
            nodes.extend(restore_form(&backups));
            if backups.is_empty() {
                nodes.push(Node::Text {
                    text: "No backups yet.".to_owned(),
                });
            }
            return Ok(View { nodes });
        }
        nodes.push(Node::Heading {
            text: "Backup".to_owned(),
        });
        nodes.push(Node::Text {
            text: format!(
                "Writes every plugin to a new file in {}. Restoring saves the plugins of a \
                 backup; it enables none.",
                store.dir().join(BACKUP_DIR).display()
            ),
        });
        nodes.push(Node::Button {
            action: BACKUP_ACTION.to_owned(),
            label: "Back up all".to_owned(),
        });
        nodes.extend(restore_form(&backups));
        nodes.push(Node::Heading {
            text: "Export".to_owned(),
        });
        nodes.push(Node::Text {
            text: "Tick the plugins to put in one bundle file, to import on another machine. \
                   An existing file is never overwritten."
                .to_owned(),
        });
        let mut fields: Vec<Field> = saved
            .iter()
            .map(|plugin| {
                boolean(
                    &format!("{EXPORT_PREFIX}{}", plugin.about.name),
                    plugin.about.name.clone(),
                )
            })
            .collect();
        fields.push(text_field(
            "file",
            format!(
                "Bundle file (empty: {}); relative to the plugin directory",
                Path::new("exports")
                    .join("personal-plugins-<time>.json")
                    .display()
            ),
        ));
        nodes.push(Node::Form {
            action: EXPORT_ACTION.to_owned(),
            submit: "Export".to_owned(),
            fields,
        });
        Ok(View { nodes })
    }

    async fn act(
        &self,
        view: &str,
        action: &str,
        input: Value,
        _: &ViewContext,
    ) -> Result<ActionOutcome, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.to_owned()));
        }
        if action == SHOW_IMPORT || action == SHOW_TRANSFER {
            self.navigate(if action == SHOW_IMPORT {
                Page::Import
            } else {
                Page::Transfer
            });
            return Ok(ActionOutcome {
                message: None,
                rerender: true,
            });
        }
        if action == LIST_ACTION {
            self.show(None);
            return Ok(ActionOutcome {
                message: None,
                rerender: true,
            });
        }
        if let Some(name) = action.strip_prefix(DETAILS_PREFIX) {
            manifest::check_name(name).map_err(UiError::InvalidInput)?;
            self.show(Some(name.to_owned()));
            return Ok(ActionOutcome {
                message: None,
                rerender: true,
            });
        }
        if let Some(name) = action.strip_prefix(DISABLE_PREFIX) {
            manifest::check_name(name).map_err(UiError::InvalidInput)?;
            let name = name.to_owned();
            return self
                .owned(|builder| async move { builder.disable(&name).await })
                .await
                .map(done);
        }
        if action == BACKUP_ACTION {
            return self.owned(backup).await.map(done);
        }
        let Value::Object(input) = input else {
            return Err(UiError::InvalidInput("the form sends an object".to_owned()));
        };
        for (prefix, reload) in [(ENABLE_PREFIX, false), (RELOAD_PREFIX, true)] {
            let Some(name) = action.strip_prefix(prefix) else {
                continue;
            };
            manifest::check_name(name).map_err(UiError::InvalidInput)?;
            if input.get(READ_FIELD) != Some(&Value::Bool(true)) {
                return Err(UiError::InvalidInput(
                    "tick the box to confirm that you read its files".to_owned(),
                ));
            }
            let name = name.to_owned();
            return self
                .owned(move |builder| async move { builder.enable(&name, reload).await })
                .await
                .map(done);
        }
        if let Some(name) = action.strip_prefix(DELETE_PREFIX) {
            manifest::check_name(name).map_err(UiError::InvalidInput)?;
            if input
                .get(CONFIRM_FIELD)
                .and_then(Value::as_str)
                .map(str::trim)
                != Some(name)
            {
                return Err(UiError::InvalidInput(format!(
                    "type `{name}` to confirm the deletion"
                )));
            }
            let name = name.to_owned();
            let outcome = self
                .owned(move |builder| async move { delete(&builder, &name).await })
                .await?;
            self.show(None);
            return Ok(done(outcome));
        }
        let replace = input.get("replace") == Some(&Value::Bool(true));
        if action == RESTORE_ACTION {
            let chosen = input.get(BACKUP_FIELD).and_then(Value::as_str);
            let Some(path) = self
                .builder
                .store
                .backup_file(chosen.unwrap_or_default())
                .await
            else {
                return Err(UiError::InvalidInput("choose a backup".to_owned()));
            };
            return self
                .owned(move |builder| async move { import(&builder, &path, replace).await })
                .await;
        }
        let file = match input.get("file") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(file)) => file.clone(),
            Some(_) => return Err(UiError::InvalidInput("`file` must be text".to_owned())),
        };
        match action {
            EXPORT_ACTION => {
                let names: Vec<String> = input
                    .iter()
                    .filter(|(_, value)| **value == Value::Bool(true))
                    .filter_map(|(key, _)| key.strip_prefix(EXPORT_PREFIX))
                    .map(str::to_owned)
                    .collect();
                if names.is_empty() {
                    return Err(UiError::InvalidInput(
                        "tick at least one plugin to export".to_owned(),
                    ));
                }
                let builder = &self.builder;
                let path = builder.store.bundle_path(&file);
                let left_out = write_bundle(builder, &names, &path).await?;
                let mut message = format!("Exported {} to {}.", names.join(", "), path.display());
                message.push_str(&left_out);
                Ok(ActionOutcome {
                    message: Some(message),
                    rerender: false,
                })
            }
            IMPORT_ACTION => {
                if file.trim().is_empty() {
                    return Err(UiError::InvalidInput("name the bundle file".to_owned()));
                }
                let path = self.builder.store.bundle_path(&file);
                let result = self
                    .owned(move |builder| async move { import(&builder, &path, replace).await })
                    .await?;
                if result.rerender {
                    self.show(None);
                }
                Ok(result)
            }
            other => Err(UiError::UnknownAction(other.to_owned())),
        }
    }
}

impl BuilderViews {
    pub(crate) fn new(builder: Arc<Builder>) -> Self {
        Self {
            builder,
            shown: Mutex::new(Page::Library),
        }
    }

    /// The page the view currently shows.
    fn shown(&self) -> Page {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn show(&self, name: Option<String>) {
        self.navigate(name.map_or(Page::Library, Page::Plugin));
    }

    fn navigate(&self, page: Page) {
        *self.shown.lock().unwrap_or_else(PoisonError::into_inner) = page;
    }

    /// Runs `work` as a task of the builder's scope and waits for it. A
    /// frontend stops waiting for an action after a while; the task still
    /// runs to its end, so a plugin is not left mounted but not kept.
    async fn owned<T, F>(&self, work: impl FnOnce(Arc<Builder>) -> F) -> Result<T, UiError>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, UiError>> + Send + 'static,
    {
        let (finished, result) = tokio::sync::oneshot::channel();
        let task = work(self.builder.clone());
        let started = self.builder.spawner.spawn(async move {
            let _ = finished.send(task.await);
        });
        if !started {
            return Err(UiError::Failed("plugin-builder is stopping".to_owned()));
        }
        result.await.unwrap_or_else(|_| {
            Err(UiError::Failed(
                "plugin-builder stopped before the action finished".to_owned(),
            ))
        })
    }

    /// The details of `plugin` with the actions its state allows.
    async fn details(&self, plugin: &Saved) -> Vec<Node> {
        let builder = &self.builder;
        let name = &plugin.about.name;
        let state = builder.state(name);
        let manifest = &plugin.manifest;
        let tools: Vec<String> = assignments(manifest)
            .into_iter()
            .map(|(tool, level)| format!("{tool} ({level})"))
            .collect();
        let hooks: Vec<String> = manifest
            .hooks
            .iter()
            .map(|hook| format!("{} (priority {})", hook.hook, hook.priority))
            .collect();
        let mut nodes = vec![
            Node::Button {
                action: LIST_ACTION.to_owned(),
                label: "Back to the list".to_owned(),
            },
            Node::Heading { text: name.clone() },
            Node::KeyValue {
                items: vec![
                    KeyValue::new("Status", state.label()),
                    KeyValue::new("Description", plugin.about.description.clone()),
                    KeyValue::new("Tools (permission)", or_none(tools)),
                    KeyValue::new("Calls services", or_none(manifest.requires.clone())),
                    KeyValue::new("Hooks", or_none(hooks)),
                    KeyValue::new("Runtime", plugin.about.runtime.label()),
                    KeyValue::new("Directory", plugin.dir.display().to_string()),
                ],
            },
        ];
        let confirm = |action: String, submit: &str| Node::Form {
            action,
            submit: submit.to_owned(),
            fields: vec![boolean(READ_FIELD, "I read its files")],
        };
        let disable = || Node::Button {
            action: format!("{DISABLE_PREFIX}{name}"),
            label: "Disable".to_owned(),
        };
        if !builder.manages() {
            if let Ok(mount) = tokio::fs::read_to_string(plugin.dir.join(MOUNT_FILE)).await {
                nodes.push(Node::Markdown {
                    text: format!("````toml\n{}\n````", mount.trim_end()),
                });
            }
        } else {
            match state {
                State::Saved => {
                    nodes.push(Node::Heading {
                        text: "Enable".to_owned(),
                    });
                    nodes.push(Node::Text {
                        text: "A plugin is a program that runs with your rights. Read the \
                               files in its directory first. Enabling starts it, assigns its \
                               tools in the policy at the permissions above, and keeps both in \
                               rah.toml."
                            .to_owned(),
                    });
                    nodes.push(confirm(format!("{ENABLE_PREFIX}{name}"), "Enable"));
                }
                State::Enabled => nodes.push(disable()),
                State::Changed | State::Stopped | State::NotRunning(_) => {
                    nodes.push(Node::Heading {
                        text: "Reload".to_owned(),
                    });
                    nodes.push(Node::Text {
                        text: "Starts the last save in place of what runs now. Read what \
                               changed first; the tools and permissions above are the new ones."
                            .to_owned(),
                    });
                    nodes.push(confirm(format!("{RELOAD_PREFIX}{name}"), "Reload"));
                    nodes.push(disable());
                }
                State::Taken => nodes.push(Node::Text {
                    text: format!(
                        "Another plugin is mounted as `{name}`. Unmount it in the Plugins \
                         panel to enable this one."
                    ),
                }),
            }
        }
        nodes.extend(delete_form(name));
        nodes
    }
}

/// Writes the plugins `names` to the new bundle `path`, with a note of the
/// ones that run. Returns a sentence naming the files left out, or nothing.
async fn write_bundle(builder: &Builder, names: &[String], path: &Path) -> Result<String, UiError> {
    let store = &builder.store;
    let mut entries = Vec::new();
    let mut left_out = Vec::new();
    for name in names {
        manifest::check_name(name).map_err(UiError::InvalidInput)?;
        let (entry, skipped) = store
            .entry(name)
            .await
            .map_err(|error| UiError::InvalidInput(format!("`{name}`: {error}")))?;
        entries.push(entry);
        left_out.extend(skipped.into_iter().map(|path| format!("{name}/{path}")));
    }
    let enabled: Vec<String> = names
        .iter()
        .filter(|name| builder.state(name).mounted())
        .cloned()
        .collect();
    store
        .write_bundle(path, entries, &enabled)
        .await
        .map_err(UiError::Failed)?;
    Ok(if left_out.is_empty() {
        String::new()
    } else {
        format!(
            " Left out (not UTF-8 text or too large): {}.",
            left_out.join(", ")
        )
    })
}

/// Writes every saved plugin to a new backup.
async fn backup(builder: Arc<Builder>) -> Result<String, UiError> {
    let names: Vec<String> = builder
        .store
        .list()
        .await
        .into_iter()
        .map(|plugin| plugin.about.name)
        .collect();
    if names.is_empty() {
        return Err(UiError::InvalidInput(
            "there are no plugins to back up".to_owned(),
        ));
    }
    let path = builder.store.backup_path().await;
    let left_out = write_bundle(&builder, &names, &path).await?;
    builder.announce();
    Ok(format!(
        "Backed up {} to {}.{left_out}",
        names.join(", "),
        path.display()
    ))
}

/// Deletes the saved plugin `name`: a backup of it is written first, and an
/// enabled plugin is disabled first.
async fn delete(builder: &Builder, name: &str) -> Result<String, UiError> {
    let _one = builder.lifecycle.lock().await;
    let state = builder.state(name);
    let copy: Result<PathBuf, String> = {
        let path = builder.store.backup_path().await;
        match write_bundle(builder, &[name.to_owned()], &path).await {
            Ok(_) => Ok(path),
            Err(error) => Err(error.to_string()),
        }
    };
    let mut notes = String::new();
    if state.mounted() {
        // A plugin that cannot be disabled keeps its files: its entry would
        // name a directory that is gone.
        notes.push_str(&format!(" {}", builder.disable_locked(name).await?));
    }
    let (dir, leftover) = builder
        .store
        .delete(name)
        .await
        .map_err(|error| match error {
            SaveError::Refused(reason) => UiError::InvalidInput(reason),
            SaveError::Failed(reason) => UiError::Failed(reason),
        })?;
    builder.announce();
    let mut message = format!("Deleted `{name}` ({}).", dir.display());
    match copy {
        Ok(path) => message.push_str(&format!(" A copy is in {}.", path.display())),
        Err(reason) => message.push_str(&format!(" No backup was written: {reason}.")),
    }
    message.push_str(&notes);
    if state == State::Taken {
        message.push_str(&format!(
            " A plugin `{name}` is still mounted: it is not this builder's mount, so remove \
             its [[plugin]] entry and policy assignments from rah.toml yourself, then restart."
        ));
    }
    if let Some(leftover) = leftover {
        message.push_str(&format!(" Remove the rest by hand: {leftover}."));
    }
    Ok(message)
}

/// Saves the plugins of the bundle at `path`. Every plugin passes the
/// checks of `plugins.save` before any is written; a plugin whose name is
/// taken is skipped unless `replace` is set. Nothing is enabled.
async fn import(builder: &Builder, path: &Path, replace: bool) -> Result<ActionOutcome, UiError> {
    let (entries, enabled) = Store::read_bundle(path)
        .await
        .map_err(UiError::InvalidInput)?;
    let mut plans = Vec::new();
    let mut names = BTreeSet::new();
    for (index, mut entry) in entries.into_iter().enumerate() {
        let label = entry.get("name").and_then(Value::as_str).map_or_else(
            || format!("plugin {}", index + 1),
            |name| format!("`{name}`"),
        );
        if let Value::Object(map) = &mut entry {
            map.insert("replace".to_owned(), Value::Bool(replace));
        }
        let plan = plan(entry, builder.max_files, builder.max_file_bytes)
            .map_err(|error| UiError::InvalidInput(format!("{label}: {error}")))?;
        if !names.insert(plan.about.name.clone()) {
            return Err(UiError::InvalidInput(format!(
                "{label} is in the bundle twice"
            )));
        }
        plans.push(plan);
    }
    if plans.is_empty() {
        return Err(UiError::InvalidInput(
            "the bundle holds no plugins".to_owned(),
        ));
    }
    let (mut imported, mut skipped, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    let mut reload = Vec::new();
    for plan in &plans {
        let program = builder.program(plan.about.runtime);
        match builder.store.save(plan, &program).await {
            Ok(_) => {
                if builder.saved_again(&plan.about.name) {
                    reload.push(plan.about.name.clone());
                }
                imported.push(plan.about.name.clone());
            }
            Err(SaveError::Refused(reason)) => {
                skipped.push(format!("{} ({reason})", plan.about.name));
            }
            Err(SaveError::Failed(reason)) => failed.push(reason),
        }
    }
    if !imported.is_empty() {
        builder.announce();
    }
    let mut message = if imported.is_empty() {
        "Imported nothing.".to_owned()
    } else {
        format!(
            "Imported {}; nothing was mounted, read the files first.",
            imported.join(", ")
        )
    };
    let were_enabled: Vec<&str> = imported
        .iter()
        .filter(|name| enabled.contains(name) && !reload.contains(name))
        .map(String::as_str)
        .collect();
    if !were_enabled.is_empty() {
        message.push_str(&format!(
            " Enabled when the bundle was written: {}.",
            were_enabled.join(", ")
        ));
    }
    if !reload.is_empty() {
        message.push_str(&format!(
            " Running an earlier save until you reload: {}.",
            reload.join(", ")
        ));
    }
    if !skipped.is_empty() {
        message.push_str(&format!(" Skipped: {}.", skipped.join("; ")));
    }
    if !failed.is_empty() {
        message.push_str(&format!(" Failed: {}.", failed.join("; ")));
    }
    Ok(ActionOutcome {
        message: Some(message),
        rerender: !imported.is_empty(),
    })
}

/// The form that deletes the plugin `name` once its name is typed.
fn delete_form(name: &str) -> [Node; 3] {
    [
        Node::Heading {
            text: "Delete".to_owned(),
        },
        Node::Text {
            text: format!(
                "Deletes the directory of `{name}` and every file in it. A backup of it is \
                 written first, and an enabled plugin is disabled first."
            ),
        },
        Node::Form {
            action: format!("{DELETE_PREFIX}{name}"),
            submit: "Delete".to_owned(),
            fields: vec![text_field(CONFIRM_FIELD, format!("Type {name} to confirm"))],
        },
    ]
}

/// The form that saves the plugins of one of `backups`; nothing without a
/// backup.
fn restore_form(backups: &[String]) -> Vec<Node> {
    let Some(newest) = backups.first() else {
        return Vec::new();
    };
    vec![Node::Form {
        action: RESTORE_ACTION.to_owned(),
        submit: "Restore".to_owned(),
        fields: vec![
            Field {
                name: BACKUP_FIELD.to_owned(),
                label: "Backup".to_owned(),
                kind: FieldKind::Choice,
                value: newest.clone(),
                options: backups.to_vec(),
            },
            boolean("replace", "Replace saved plugins of the same name"),
        ],
    }]
}

/// The form that imports a bundle file.
fn import_form() -> [Node; 2] {
    [
        Node::Heading {
            text: "Import".to_owned(),
        },
        Node::Form {
            action: IMPORT_ACTION.to_owned(),
            submit: "Import".to_owned(),
            fields: vec![
                text_field("file", "Bundle file; relative to the plugin directory"),
                boolean("replace", "Replace saved plugins of the same name"),
            ],
        },
    ]
}
