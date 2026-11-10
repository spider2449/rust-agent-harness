//! Enabling, reloading, disabling, deleting and backing up from the view
//! (ADR 0073), against an `admin` service that records what it is asked.

use std::collections::BTreeMap;

use rah_api::{
    PluginId, PluginStatus,
    admin::{ADMIN, AdminError, ConfigChange, MountRequest, PluginAdmin, PluginInstance},
};

use super::*;
use crate::store::BACKUP_DIR;

const POLICY: &str = "policy-authority";

/// A plugin that does nothing, standing in for whatever the admin mounts.
struct Stub(PluginManifest);

#[async_trait]
impl Plugin for Stub {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, _: &mut Scope) -> Result<(), PluginError> {
        Ok(())
    }
}

/// An `admin` service that mounts stubs and records the rest.
struct FakeAdmin {
    kernel: KernelHandle,
    records: Mutex<BTreeMap<PluginId, (String, Value)>>,
    kept: Mutex<Vec<ConfigChange>>,
    /// When set, a mount is refused with this reason.
    refuse_mount: Mutex<Option<String>>,
}

impl FakeAdmin {
    fn config(&self, id: &str) -> Value {
        self.records.lock().unwrap()[&PluginId::from(id)].1.clone()
    }

    fn kept(&self) -> Vec<ConfigChange> {
        std::mem::take(&mut self.kept.lock().unwrap())
    }

    fn refuse_mounts(&self, reason: Option<&str>) {
        *self.refuse_mount.lock().unwrap() = reason.map(str::to_owned);
    }
}

#[async_trait]
impl PluginAdmin for FakeAdmin {
    fn kinds(&self) -> Vec<String> {
        vec!["process".to_owned(), POLICY.to_owned()]
    }

    fn instance(&self, id: &PluginId) -> Option<PluginInstance> {
        let records = self.records.lock().unwrap();
        let (kind, config) = records.get(id)?;
        Some(PluginInstance {
            kind: kind.clone(),
            config: config.clone(),
            locked: false,
            stopped: false,
        })
    }

    async fn mount(&self, request: MountRequest) -> Result<(PluginId, PluginStatus), AdminError> {
        if let Some(reason) = self.refuse_mount.lock().unwrap().clone() {
            return Err(AdminError::Failed(reason));
        }
        let id = PluginId::from(request.id.clone().unwrap_or_else(|| request.kind.clone()));
        if self.kernel.status(&id).is_some() {
            return Err(AdminError::AlreadyMounted(id));
        }
        let status = self
            .kernel
            .mount(
                Arc::new(Stub(PluginManifest::new(id.clone(), "0.1.0"))),
                MountOptions::default(),
            )
            .await
            .map_err(|error| AdminError::Failed(error.to_string()))?;
        self.records
            .lock()
            .unwrap()
            .insert(id.clone(), (request.kind, request.config));
        Ok((id, status))
    }

    async fn unmount(&self, id: &PluginId) -> Result<(), AdminError> {
        self.kernel
            .unmount(id)
            .await
            .map_err(|error| AdminError::Failed(error.to_string()))?;
        self.records.lock().unwrap().remove(id);
        Ok(())
    }

    async fn configure(&self, id: &PluginId, config: Value) -> Result<PluginStatus, AdminError> {
        match self.records.lock().unwrap().get_mut(id) {
            Some(record) => record.1 = config,
            None => return Err(AdminError::NotMounted(id.clone())),
        }
        Ok(PluginStatus::Active)
    }

    async fn restart(&self, id: &PluginId) -> Result<PluginStatus, AdminError> {
        Err(AdminError::NotMounted(id.clone()))
    }

    async fn stop(&self, id: &PluginId) -> Result<(), AdminError> {
        Err(AdminError::NotMounted(id.clone()))
    }

    async fn start(&self, id: &PluginId) -> Result<PluginStatus, AdminError> {
        Err(AdminError::NotMounted(id.clone()))
    }

    fn stopped(&self) -> Vec<PluginId> {
        Vec::new()
    }

    async fn replace(
        &self,
        old: &PluginId,
        _: MountRequest,
    ) -> Result<(PluginId, PluginStatus), AdminError> {
        Err(AdminError::NotMounted(old.clone()))
    }

    async fn persist(&self, change: ConfigChange) -> Result<PathBuf, AdminError> {
        self.kept.lock().unwrap().push(change);
        Ok(PathBuf::from("rah.toml"))
    }
}

/// Provides a [`FakeAdmin`] as the `admin` service.
struct AdminPlugin(PluginManifest, Arc<FakeAdmin>);

#[async_trait]
impl Plugin for AdminPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let admin: Arc<dyn PluginAdmin> = self.1.clone();
        scope.provide(ADMIN, admin)?;
        Ok(())
    }
}

/// A kernel with an admin, a policy holding `tools`, and the builder with
/// an interpreter it can find.
async fn managed(dir: &Path, tools: Value) -> (Kernel, Arc<FakeAdmin>) {
    let kernel = Kernel::new();
    let admin = Arc::new(FakeAdmin {
        kernel: kernel.handle(),
        records: Mutex::new(BTreeMap::new()),
        kept: Mutex::new(Vec::new()),
        refuse_mount: Mutex::new(None),
    });
    kernel
        .mount(
            Arc::new(AdminPlugin(
                PluginManifest::new("admin", "0.1.0").provides(["admin"]),
                admin.clone(),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    admin
        .mount(MountRequest {
            kind: POLICY.to_owned(),
            config: json!({ "tools": tools }),
            ..MountRequest::default()
        })
        .await
        .unwrap();
    // An interpreter the builder finds, so a mount has a program.
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("py3.exe"), b"").unwrap();
    let search = crate::programs::Search {
        path: vec![bin],
        extensions: vec![".exe".to_owned()],
        uv_python: Vec::new(),
    };
    kernel
        .mount(
            Arc::new(PluginBuilderPlugin::new("plugin-builder").with_search(search)),
            mount_options(&dir.join("plugins")),
        )
        .await
        .unwrap();
    (kernel, admin)
}

fn ticked() -> Value {
    json!({ READ_FIELD: true })
}

fn status_of(view: &View, name: &str) -> String {
    view.nodes
        .iter()
        .find_map(|node| match node {
            Node::Table { rows, .. } => rows
                .iter()
                .find(|row| row[0] == name)
                .map(|row| row[1].clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no row for {name}"))
}

/// The actions the view offers, buttons and forms alike.
fn actions(view: &View) -> Vec<String> {
    view.nodes
        .iter()
        .filter_map(|node| match node {
            Node::Button { action, .. } | Node::Form { action, .. } => Some(action.clone()),
            _ => None,
        })
        .collect()
}

fn mounted(kernel: &Kernel, id: &str) -> bool {
    kernel.handle().status(&PluginId::from(id)).is_some()
}

#[tokio::test]
async fn enabling_assigns_the_tools_mounts_the_plugin_and_keeps_both() {
    let dir = TempDir::new("enable");
    let (kernel, admin) = managed(&dir.0, json!({ "echo": "none" })).await;
    let saved = invoke(&kernel, SAVE_TOOL, greeter()).await;
    let reply = output_text(&saved);
    assert!(
        reply.contains("It is not enabled and has not run"),
        "{reply}"
    );
    assert!(reply.contains("\"My plugins\" panel"), "{reply}");
    assert_eq!(status_of(&render(&kernel).await, "greeter"), "not enabled");
    assert!(!mounted(&kernel, "greeter"));

    // The details offer to enable it, with the permissions to confirm.
    act(&kernel, "details:greeter", Value::Null).await.unwrap();
    let details = render(&kernel).await;
    assert!(details.nodes.iter().any(|node| matches!(
        node,
        Node::KeyValue { items }
            if items.contains(&KeyValue::new("Tools (permission)", "greeter.hello (none)"))
                && items.contains(&KeyValue::new("Calls services", "storage"))
    )));
    assert_eq!(
        actions(&details),
        ["list", "enable:greeter", "delete:greeter"]
    );

    // Without the tick nothing happens.
    for input in [json!({}), json!({ READ_FIELD: false })] {
        assert!(matches!(
            act(&kernel, "enable:greeter", input).await,
            Err(UiError::InvalidInput(reason)) if reason.contains("tick the box")
        ));
    }
    assert!(!mounted(&kernel, "greeter"));
    assert!(admin.kept().is_empty());

    let enabled = act(&kernel, "enable:greeter", ticked()).await.unwrap();
    assert!(enabled.rerender);
    let message = enabled.message.unwrap();
    assert!(message.starts_with("Enabled `greeter`."), "{message}");
    assert!(message.contains("greeter.hello = none"), "{message}");
    assert!(!message.contains("not kept"), "{message}");
    assert_eq!(
        kernel.handle().status(&PluginId::from("greeter")),
        Some(PluginStatus::Active)
    );
    // The policy got the assignment and kept its own.
    assert_eq!(
        admin.config(POLICY)["tools"],
        json!({ "echo": "none", "greeter.hello": "none" })
    );
    // It runs in the plugin directory, not in its own.
    let plugins = std::path::absolute(dir.0.join("plugins")).unwrap();
    let config = admin.config("greeter");
    assert_eq!(
        config,
        json!({
            "program": dir.0.join("bin").join("py3.exe").display().to_string(),
            "args": [plugins.join("greeter").join("plugin.py").display().to_string()],
            "cwd": plugins.display().to_string(),
            "personal": true,
        })
    );
    assert_eq!(
        admin.kept(),
        [
            ConfigChange::Plugin {
                id: PluginId::from(POLICY),
                kind: POLICY.to_owned(),
                config: admin.config(POLICY),
            },
            ConfigChange::Plugin {
                id: PluginId::from("greeter"),
                kind: "process".to_owned(),
                config,
            },
        ]
    );
    let details = render(&kernel).await;
    assert_eq!(
        actions(&details),
        ["list", "disable:greeter", "delete:greeter"]
    );
    assert!(matches!(
        act(&kernel, "enable:greeter", ticked()).await,
        Err(UiError::InvalidInput(reason)) if reason.contains("already enabled")
    ));
    act(&kernel, LIST_ACTION, Value::Null).await.unwrap();
    assert_eq!(status_of(&render(&kernel).await, "greeter"), "enabled");
}

#[tokio::test]
async fn a_plugin_that_does_not_start_leaves_nothing_behind() {
    let dir = TempDir::new("enable-fails");
    let (kernel, admin) = managed(&dir.0, json!({ "echo": "none" })).await;
    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    admin.refuse_mounts(Some("the plugin exited at once"));
    assert!(matches!(
        act(&kernel, "enable:greeter", ticked()).await,
        Err(UiError::Failed(reason))
            if reason.contains("was not enabled") && reason.contains("exited at once")
    ));
    assert!(!mounted(&kernel, "greeter"));
    assert_eq!(admin.config(POLICY)["tools"], json!({ "echo": "none" }));
    assert!(admin.kept().is_empty());
    assert_eq!(status_of(&render(&kernel).await, "greeter"), "not enabled");
}

#[tokio::test]
async fn an_assignment_the_policy_has_is_never_changed() {
    let dir = TempDir::new("enable-keeps");
    let (kernel, admin) = managed(&dir.0, json!({ "greeter.hello": "write" })).await;
    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    act(&kernel, "enable:greeter", ticked()).await.unwrap();
    assert_eq!(
        admin.config(POLICY)["tools"],
        json!({ "greeter.hello": "write" })
    );
    // Only the plugin was kept: the policy did not change.
    assert!(matches!(
        admin.kept().as_slice(),
        [ConfigChange::Plugin { kind, .. }] if kind == "process"
    ));
    // Disabling leaves an assignment that is not the declared level.
    act(&kernel, "disable:greeter", Value::Null).await.unwrap();
    assert_eq!(
        admin.config(POLICY)["tools"],
        json!({ "greeter.hello": "write" })
    );
}

#[tokio::test]
async fn a_saved_change_runs_after_a_reload() {
    let dir = TempDir::new("reload");
    let (kernel, admin) = managed(&dir.0, json!({})).await;
    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    // Nothing to reload before it is enabled.
    assert!(matches!(
        act(&kernel, "reload:greeter", ticked()).await,
        Err(UiError::InvalidInput(reason)) if reason.contains("not enabled")
    ));
    act(&kernel, "enable:greeter", ticked()).await.unwrap();
    admin.kept();

    // The model saves it again with one more tool; the earlier save runs on.
    let changed = greeter_with(|input| {
        input["replace"] = json!(true);
        input["manifest"]["tools"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "name": "greeter.shout",
                "description": "Greets loudly.",
                "input_schema": { "type": "object" },
                "permission": "write"
            }));
    });
    let reply = output_text(&invoke(&kernel, SAVE_TOOL, changed).await);
    assert!(reply.contains("still the earlier save"), "{reply}");
    assert!(reply.contains("press Reload"), "{reply}");
    assert!(mounted(&kernel, "greeter"));
    assert_eq!(
        status_of(&render(&kernel).await, "greeter"),
        "enabled; reload to run the last save"
    );
    assert_eq!(
        admin.config(POLICY)["tools"],
        json!({ "greeter.hello": "none" })
    );
    act(&kernel, "details:greeter", Value::Null).await.unwrap();
    assert_eq!(
        actions(&render(&kernel).await),
        [
            "list",
            "reload:greeter",
            "disable:greeter",
            "delete:greeter"
        ]
    );
    assert!(matches!(
        act(&kernel, "reload:greeter", json!({})).await,
        Err(UiError::InvalidInput(_))
    ));

    let reloaded = act(&kernel, "reload:greeter", ticked()).await.unwrap();
    assert!(reloaded.message.unwrap().starts_with("Reloaded `greeter`."));
    assert_eq!(
        admin.config(POLICY)["tools"],
        json!({ "greeter.hello": "none", "greeter.shout": "write" })
    );
    assert_eq!(admin.kept().len(), 2);
    act(&kernel, LIST_ACTION, Value::Null).await.unwrap();
    assert_eq!(status_of(&render(&kernel).await, "greeter"), "enabled");

    // A save that does not start again leaves the plugin disabled, and out
    // of the file, so the next start of rah does not fail on it.
    let again = greeter_with(|input| input["replace"] = json!(true));
    assert!(!invoke(&kernel, SAVE_TOOL, again).await.is_error);
    admin.refuse_mounts(Some("syntax error"));
    assert!(matches!(
        act(&kernel, "reload:greeter", ticked()).await,
        Err(UiError::Failed(reason))
            if reason.contains("is now disabled") && reason.contains("syntax error")
    ));
    assert!(!mounted(&kernel, "greeter"));
    assert_eq!(
        admin.kept(),
        [ConfigChange::Removed {
            id: PluginId::from("greeter")
        }]
    );
    assert_eq!(status_of(&render(&kernel).await, "greeter"), "not enabled");
}

#[tokio::test]
async fn disabling_unmounts_the_plugin_and_takes_its_assignments_back() {
    let dir = TempDir::new("disable");
    let (kernel, admin) = managed(&dir.0, json!({ "echo": "none" })).await;
    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    assert!(matches!(
        act(&kernel, "disable:greeter", Value::Null).await,
        Err(UiError::InvalidInput(reason)) if reason.contains("not enabled")
    ));
    act(&kernel, "enable:greeter", ticked()).await.unwrap();
    admin.kept();

    let disabled = act(&kernel, "disable:greeter", Value::Null).await.unwrap();
    let message = disabled.message.unwrap();
    assert!(message.starts_with("Disabled `greeter`"), "{message}");
    assert!(!mounted(&kernel, "greeter"));
    assert_eq!(admin.config(POLICY)["tools"], json!({ "echo": "none" }));
    assert_eq!(
        admin.kept(),
        [
            ConfigChange::Removed {
                id: PluginId::from("greeter")
            },
            ConfigChange::Plugin {
                id: PluginId::from(POLICY),
                kind: POLICY.to_owned(),
                config: admin.config(POLICY),
            },
        ]
    );
    // The files stay, and it can be enabled again.
    assert!(dir.0.join("plugins/greeter/plugin.py").is_file());
    assert_eq!(status_of(&render(&kernel).await, "greeter"), "not enabled");
    act(&kernel, "enable:greeter", ticked()).await.unwrap();
    assert!(mounted(&kernel, "greeter"));
}

#[tokio::test]
async fn deleting_an_enabled_plugin_disables_it_and_keeps_a_copy() {
    let dir = TempDir::new("delete-enabled");
    let (kernel, admin) = managed(&dir.0, json!({})).await;
    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    act(&kernel, "enable:greeter", ticked()).await.unwrap();
    admin.kept();

    let deleted = act(
        &kernel,
        "delete:greeter",
        json!({ CONFIRM_FIELD: "greeter" }),
    )
    .await
    .unwrap();
    let message = deleted.message.unwrap();
    assert!(message.starts_with("Deleted `greeter`"), "{message}");
    assert!(message.contains("A copy is in"), "{message}");
    assert!(message.contains("Disabled `greeter`"), "{message}");
    assert!(!message.contains("still mounted"), "{message}");
    assert!(!mounted(&kernel, "greeter"));
    assert!(!dir.0.join("plugins/greeter").exists());
    assert_eq!(admin.config(POLICY)["tools"], json!({}));
    assert!(admin.kept().contains(&ConfigChange::Removed {
        id: PluginId::from("greeter")
    }));

    // The copy holds the plugin and notes that it ran.
    let backups: Vec<PathBuf> = std::fs::read_dir(dir.0.join("plugins").join(BACKUP_DIR))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(backups.len(), 1);
    let bundle: Value = serde_json::from_slice(&std::fs::read(&backups[0]).unwrap()).unwrap();
    assert_eq!(bundle["plugins"][0]["name"], json!("greeter"));
    assert_eq!(bundle["enabled"], json!(["greeter"]));
}

#[tokio::test]
async fn a_backup_is_restored_from_the_list_and_enables_nothing() {
    let dir = TempDir::new("backup");
    let (kernel, _admin) = managed(&dir.0, json!({})).await;
    // Nothing to back up, and no restore form without a backup.
    assert!(matches!(
        act(&kernel, BACKUP_ACTION, Value::Null).await,
        Err(UiError::InvalidInput(reason)) if reason.contains("no plugins")
    ));
    assert!(!actions(&render(&kernel).await).contains(&RESTORE_ACTION.to_owned()));

    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    let farewell = greeter_with(|input| {
        input["name"] = json!("farewell");
        input["manifest"]["tools"][0]["name"] = json!("farewell.bye");
    });
    assert!(!invoke(&kernel, SAVE_TOOL, farewell).await.is_error);
    act(&kernel, "enable:greeter", ticked()).await.unwrap();

    let backed_up = act(&kernel, BACKUP_ACTION, Value::Null).await.unwrap();
    let message = backed_up.message.unwrap();
    assert!(
        message.starts_with("Backed up farewell, greeter to "),
        "{message}"
    );
    // A second backup in the same second gets its own file.
    act(&kernel, BACKUP_ACTION, Value::Null).await.unwrap();
    let view = render(&kernel).await;
    let Some(Node::Form { fields, .. }) = view
        .nodes
        .iter()
        .find(|node| matches!(node, Node::Form { action, .. } if action == RESTORE_ACTION))
    else {
        panic!("no restore form");
    };
    assert_eq!(fields[0].kind, FieldKind::Choice);
    assert_eq!(fields[0].options.len(), 2);
    assert!(fields[0].options.contains(&fields[0].value));
    let backup = fields[0].value.clone();

    // Both plugins go; the restore brings them back, not enabled.
    for name in ["greeter", "farewell"] {
        act(
            &kernel,
            &format!("delete:{name}"),
            json!({ CONFIRM_FIELD: name }),
        )
        .await
        .unwrap();
    }
    assert!(!mounted(&kernel, "greeter"));
    // Only a listed backup is read; a path is not.
    for chosen in [json!("../greeter/manifest.json"), json!(""), Value::Null] {
        assert!(matches!(
            act(&kernel, RESTORE_ACTION, json!({ "backup": chosen })).await,
            Err(UiError::InvalidInput(reason)) if reason.contains("choose a backup")
        ));
    }
    let restored = act(&kernel, RESTORE_ACTION, json!({ "backup": backup }))
        .await
        .unwrap();
    let message = restored.message.unwrap();
    assert!(
        message.starts_with("Imported farewell, greeter;"),
        "{message}"
    );
    assert!(
        message.contains("Enabled when the bundle was written: greeter."),
        "{message}"
    );
    assert!(dir.0.join("plugins/greeter/plugin.py").is_file());
    assert!(!mounted(&kernel, "greeter"));
    let view = render(&kernel).await;
    assert_eq!(status_of(&view, "greeter"), "not enabled");
    assert_eq!(status_of(&view, "farewell"), "not enabled");
    // Restoring over saved plugins needs the replace box.
    let again = act(&kernel, RESTORE_ACTION, json!({ "backup": backup }))
        .await
        .unwrap();
    assert!(again.message.unwrap().starts_with("Imported nothing."));
}

#[tokio::test]
async fn without_an_admin_or_with_the_name_taken_nothing_is_mounted() {
    // No admin service: mount.toml is the way.
    let dir = TempDir::new("no-admin");
    let kernel = Kernel::new();
    mount_builder(&kernel, &dir.0).await;
    let reply = output_text(&invoke(&kernel, SAVE_TOOL, greeter()).await);
    assert!(
        reply.contains("follow the steps in `mount.toml`"),
        "{reply}"
    );
    assert!(matches!(
        act(&kernel, "enable:greeter", ticked()).await,
        Err(UiError::Failed(reason)) if reason.contains("mount.toml")
    ));
    // mount.toml runs the plugin in the plugin directory too.
    let mount = std::fs::read_to_string(dir.0.join("greeter/mount.toml")).unwrap();
    let cwd = toml_text(&std::path::absolute(&dir.0).unwrap());
    assert!(mount.contains(&format!("cwd = {cwd},")), "{mount}");

    // Another plugin holds the name: it is not touched.
    let dir = TempDir::new("taken");
    let (kernel, admin) = managed(&dir.0, json!({})).await;
    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    admin
        .mount(MountRequest {
            kind: "tools-http".to_owned(),
            id: Some("greeter".to_owned()),
            ..MountRequest::default()
        })
        .await
        .unwrap();
    assert_eq!(
        status_of(&render(&kernel).await, "greeter"),
        "its name is taken by another plugin"
    );
    for action in ["enable:greeter", "reload:greeter"] {
        assert!(matches!(
            act(&kernel, action, ticked()).await,
            Err(UiError::InvalidInput(reason)) if reason.contains("another plugin")
        ));
    }
    assert!(matches!(
        act(&kernel, "disable:greeter", Value::Null).await,
        Err(UiError::InvalidInput(_))
    ));
    assert!(mounted(&kernel, "greeter"));
    assert_eq!(admin.config("greeter"), Value::Null);
}

/// `path` as `mount.toml` writes it: a TOML basic string.
fn toml_text(path: &Path) -> String {
    format!("\"{}\"", path.display().to_string().replace('\\', "\\\\"))
}
