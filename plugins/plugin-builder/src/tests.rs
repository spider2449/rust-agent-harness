use std::{path::Path, sync::Mutex};

use rah_api::{
    CancellationToken, Kernel, MountOptions,
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{SessionId, ToolCall, ToolCallId, ToolInput};

use super::*;
use crate::store::{ABOUT_FILE, MANIFEST_FILE, check_path};

/// A directory under the system temp directory, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "rah-plugin-builder-{name}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn greeter() -> Value {
    json!({
        "name": "greeter",
        "description": "  Greets people\n by name. ",
        "runtime": "python",
        "manifest": {
            "version": "0.1.0",
            "requires": ["storage"],
            "tools": [{
                "name": "greeter.hello",
                "description": "Greets someone.",
                "input_schema": {
                    "type": "object",
                    "properties": { "name": { "type": "string" } },
                    "required": ["name"]
                },
                "permission": "none"
            }],
            "views": [{ "id": "board", "title": "Greetings" }]
        },
        "files": [
            { "path": "plugin.py", "content": "print('hi')\n" },
            { "path": "lib/names.py", "content": "NAMES = []\n" }
        ]
    })
}

/// `greeter()` with `edit` applied to it.
fn greeter_with(edit: impl FnOnce(&mut Value)) -> Value {
    let mut input = greeter();
    edit(&mut input);
    input
}

#[test]
fn a_plugin_that_follows_the_rules_is_planned() {
    let planned = plan(greeter(), 16, 1024).unwrap();
    assert_eq!(planned.about.name, "greeter");
    assert_eq!(planned.about.description, "Greets people by name.");
    assert_eq!(planned.about.runtime, Runtime::Python);
    assert_eq!(planned.files.len(), 2);
    assert!(!planned.replace);
    assert_eq!(
        planned.manifest.to_file()["provides"],
        json!(["tools", "ui-views"])
    );
    // Hooks only, with a priority below the policy's.
    let hooks = greeter_with(|input| {
        input["manifest"] = json!({
            "version": "1",
            "hooks": [{ "hook": "tool.before_invoke", "priority": -10 }, { "hook": "tool.after_invoke" }],
            "events": ["session.appended"]
        });
    });
    let hooked = plan(hooks, 16, 1024).unwrap();
    assert_eq!(hooked.manifest.to_file()["provides"], json!([]));
}

#[test]
fn plugins_that_break_a_rule_are_refused() {
    type Edit = fn(&mut Value);
    let cases: [(&str, Edit); 26] = [
        ("unknown field", |i| i["extra"] = json!(1)),
        ("uppercase name", |i| i["name"] = json!("Greeter")),
        ("name with a dot", |i| i["name"] = json!("greet.er")),
        ("name ending in -", |i| i["name"] = json!("greeter-")),
        ("reserved name", |i| i["name"] = json!("repo")),
        ("empty description", |i| i["description"] = json!(" \n ")),
        ("long description", |i| {
            i["description"] = json!("x".repeat(201))
        }),
        ("unknown runtime", |i| i["runtime"] = json!("ruby")),
        ("provides is derived", |i| {
            i["manifest"]["provides"] = json!(["tools"])
        }),
        ("protocol is derived", |i| {
            i["manifest"]["protocol"] = json!(4)
        }),
        ("empty version", |i| i["manifest"]["version"] = json!("")),
        ("nothing declared", |i| {
            i["manifest"] = json!({ "version": "1" });
        }),
        ("uncallable service", |i| {
            i["manifest"]["requires"] = json!(["model"])
        }),
        ("service twice", |i| {
            i["manifest"]["requires"] = json!(["storage", "storage"]);
        }),
        ("tool outside the namespace", |i| {
            i["manifest"]["tools"][0]["name"] = json!("fs.read");
        }),
        ("tool name with whitespace", |i| {
            i["manifest"]["tools"][0]["name"] = json!("greeter.say hello");
        }),
        ("tool without description", |i| {
            i["manifest"]["tools"][0]["description"] = json!("");
        }),
        ("schema not an object", |i| {
            i["manifest"]["tools"][0]["input_schema"] = json!({ "type": "string" });
        }),
        ("unknown permission", |i| {
            i["manifest"]["tools"][0]["permission"] = json!("admin");
        }),
        ("policy-first hook", |i| {
            i["manifest"]["hooks"] = json!([{ "hook": "tool.before_invoke" }]);
        }),
        ("unknown hook", |i| {
            i["manifest"]["hooks"] = json!([{ "hook": "tool.approval", "priority": -1 }]);
        }),
        ("unknown event", |i| {
            i["manifest"]["events"] = json!(["turn.ended"])
        }),
        ("invalid view id", |i| {
            i["manifest"]["views"] = json!([{ "id": "Board", "title": "B" }]);
        }),
        ("missing entry file", |i| i["runtime"] = json!("node")),
        ("reserved file", |i| {
            i["files"][1] = json!({ "path": "Manifest.json", "content": "{}" });
        }),
        ("file and directory", |i| {
            i["files"][1] = json!({ "path": "plugin.py/x.py", "content": "" });
        }),
    ];
    for (case, edit) in cases {
        let input = greeter_with(edit);
        assert!(plan(input, 16, 1024).is_err(), "{case} was accepted");
    }
    // Limits from the configuration.
    assert!(plan(greeter(), 1, 1024).is_err());
    assert!(plan(greeter(), 16, 4).is_err());
    let twice = greeter_with(|i| {
        i["files"][1] = json!({ "path": "PLUGIN.PY", "content": "" });
    });
    assert!(plan(twice, 16, 1024).is_err());
}

#[test]
fn file_paths_stay_inside_the_plugin() {
    for good in [
        "plugin.py",
        "lib/util.py",
        "a/b/c/d.txt",
        "README.md",
        "data-1_x.json",
    ] {
        assert!(check_path(good).is_ok(), "{good}");
    }
    for bad in [
        "",
        "../x.py",
        "lib/../../x",
        "/etc/passwd",
        "C:/x.py",
        "lib\\x.py",
        ".env",
        "lib/.hidden",
        "a/b/c/d/e.txt",
        "lib//x.py",
        "x.",
        "con.txt",
        "lib/COM1",
        "with space.py",
        "mount.toml",
        "rah_plugin.mjs",
    ] {
        assert!(check_path(bad).is_err(), "{bad}");
    }
    // Reserved names are only reserved at the top.
    assert!(check_path("data/manifest.json").is_ok());
}

fn mount_options(dir: &Path) -> MountOptions {
    MountOptions {
        config: json!({ "dir": dir, "python": "py3", "node": "node" }),
        ..MountOptions::default()
    }
}

async fn mount_builder(kernel: &Kernel, dir: &Path) {
    kernel
        .mount(
            Arc::new(
                PluginBuilderPlugin::new("plugin-builder")
                    .with_search(crate::programs::Search::default()),
            ),
            mount_options(dir),
        )
        .await
        .unwrap();
}

async fn invoke(kernel: &Kernel, name: &str, input: Value) -> ToolOutput {
    ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(name),
                    input: ToolInput(input),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
        .await
        .output
}

fn output_text(output: &ToolOutput) -> String {
    output
        .content
        .iter()
        .map(|item| match item {
            ToolContent::Text(text) => text.clone(),
            ToolContent::Json(value) => value.to_string(),
            other => panic!("unexpected content {other:?}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn output_json(output: &ToolOutput) -> Value {
    match output.content.first() {
        Some(ToolContent::Json(value)) => value.clone(),
        other => panic!("unexpected content {other:?}"),
    }
}

async fn render(kernel: &Kernel) -> View {
    let all = kernel.handle().get_all(UI_VIEWS);
    let (_, views) = all.iter().find(|(id, _)| id == "plugin-builder").unwrap();
    let view = views.render(VIEW, &ViewContext::default()).await.unwrap();
    assert!(view.check().is_ok());
    view
}

#[tokio::test]
async fn the_tools_and_their_permissions() {
    let dir = TempDir::new("tools");
    let kernel = Kernel::new();
    mount_builder(&kernel, &dir.0).await;
    let tools: Vec<_> = ToolInvoker::new(kernel.handle())
        .definitions()
        .into_iter()
        .map(|definition| (definition.name.as_str().to_owned(), definition.permission))
        .collect();
    assert_eq!(
        tools,
        [
            (GUIDE_TOOL.to_owned(), PermissionLevel::None),
            (SAVE_TOOL.to_owned(), PermissionLevel::Write),
            (LIST_TOOL.to_owned(), PermissionLevel::Read),
            (READ_TOOL.to_owned(), PermissionLevel::Read),
        ]
    );
    let guide = output_text(&invoke(&kernel, GUIDE_TOOL, json!({})).await);
    assert!(guide.starts_with("# Writing a personal RAH plugin"));
    assert!(guide.contains(&dir.0.display().to_string()));
    // Neither interpreter is found in the empty search: the guide says so.
    assert!(
        guide.contains("- `python` plugins: `py3` was not found on PATH;"),
        "{guide}"
    );
    assert!(
        guide.contains("- `node` plugins: `node` was not found"),
        "{guide}"
    );
    assert!(
        invoke(&kernel, GUIDE_TOOL, json!({ "x": 1 }))
            .await
            .is_error
    );
}

#[tokio::test]
async fn model_requests_say_a_plugin_means_a_personal_rah_plugin() {
    use rah_api::hooks::ModelBeforeRequest;
    use rah_protocol::{ContextItem, ModelRequest};

    let dir = TempDir::new("hint");
    let kernel = Kernel::new();
    mount_builder(&kernel, &dir.0).await;
    let request = ModelRequest {
        items: vec![
            ContextItem::System {
                text: "prompt".into(),
            },
            ContextItem::User {
                text: "write me a plugin".into(),
            },
        ],
        ..ModelRequest::default()
    };
    let items = kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request)
        .await
        .unwrap()
        .items;
    assert_eq!(items.len(), 3);
    assert_eq!(
        items[1],
        ContextItem::System {
            text: HINT.to_owned()
        }
    );
    assert!(HINT.contains(GUIDE_TOOL) && HINT.contains(SAVE_TOOL));
    assert!(matches!(&items[2], ContextItem::User { .. }));
}

#[tokio::test]
async fn a_saved_plugin_can_be_listed_read_changed_and_mounted_by_the_user() {
    let dir = TempDir::new("save");
    let kernel = Kernel::new();
    mount_builder(&kernel, &dir.0).await;
    let changed = Arc::new(Mutex::new(Vec::new()));
    let _subscription = {
        let changed = changed.clone();
        kernel
            .handle()
            .subscribe(move |event: &UiChanged| changed.lock().unwrap().push(event.clone()))
    };

    assert!(output_text(&invoke(&kernel, LIST_TOOL, json!({})).await).starts_with("No personal"));
    let saved = invoke(&kernel, SAVE_TOOL, greeter()).await;
    assert!(!saved.is_error, "{}", output_text(&saved));
    let reply = output_text(&saved);
    assert!(
        reply.contains("It is not mounted and has not run"),
        "{reply}"
    );
    assert!(reply.contains("needs the user's attention"), "{reply}");

    let plugin = dir.0.join("greeter");
    let read = |path: &str| std::fs::read_to_string(plugin.join(path)).unwrap();
    assert_eq!(read("plugin.py"), "print('hi')\n");
    assert_eq!(read("lib/names.py"), "NAMES = []\n");
    assert_eq!(read("rah_plugin.py"), include_str!("runtime/rah_plugin.py"));
    assert!(!plugin.join("rah_plugin.mjs").exists());
    let manifest: Value = serde_json::from_str(&read(MANIFEST_FILE)).unwrap();
    assert_eq!(manifest["provides"], json!(["tools", "ui-views"]));
    assert_eq!(manifest["requires"], json!(["storage"]));
    assert!(manifest.get("protocol").is_none() && manifest.get("hooks").is_none());
    let about: Value = serde_json::from_str(&read(ABOUT_FILE)).unwrap();
    assert_eq!(
        about,
        json!({ "name": "greeter", "description": "Greets people by name.", "runtime": "python" })
    );
    let mount = read(MOUNT_FILE);
    let entry = std::path::absolute(plugin.join("plugin.py")).unwrap();
    for expected in [
        "[[plugin]]\nuse = \"process\"\nid = \"greeter\"\n".to_owned(),
        format!(
            "config = {{ program = \"py3\", args = [\"{}\"]",
            entry.display().to_string().replace('\\', "\\\\")
        ),
        "# WARNING: `py3` was not found on PATH; set `program`".to_owned(),
        "# 2. REQUIRED: add its tools".to_owned(),
        "#      \"greeter.hello\" = \"none\"\n".to_owned(),
        "tools = { echo = \"none\", ..., \"greeter.hello\" = \"none\" }".to_owned(),
        "# It calls these host services: storage.".to_owned(),
        "# Its Web UI panels: greeter/board.".to_owned(),
        ", personal = true }\n".to_owned(),
    ] {
        assert!(mount.contains(&expected), "{expected} not in\n{mount}");
    }
    assert!(reply.contains(&mount));
    // No staging directory is left behind.
    let names: Vec<_> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["greeter"]);
    assert_eq!(
        *changed.lock().unwrap(),
        [UiChanged {
            instance: "plugin-builder".into(),
            view: VIEW.into(),
        }]
    );

    // The same name is refused without `replace`, and nothing changes.
    let again = invoke(&kernel, SAVE_TOOL, greeter()).await;
    assert!(again.is_error && output_text(&again).contains("replace: true"));
    assert_eq!(changed.lock().unwrap().len(), 1);

    let listed = output_json(&invoke(&kernel, LIST_TOOL, json!({})).await);
    assert_eq!(listed["plugins"][0]["name"], json!("greeter"));
    assert_eq!(
        listed["plugins"][0]["tools"],
        json!([{ "name": "greeter.hello", "permission": "none" }])
    );
    assert_eq!(listed["plugins"][0]["views"], json!(["board"]));

    // `plugins.read` returns what `plugins.save` takes; saved again with a
    // change, the plugin is replaced whole.
    let mut read_back =
        output_json(&invoke(&kernel, READ_TOOL, json!({ "name": "greeter" })).await);
    assert_eq!(
        read_back["files"],
        json!([
            { "path": "lib/names.py", "content": "NAMES = []\n" },
            { "path": "plugin.py", "content": "print('hi')\n" }
        ])
    );
    read_back["files"] = json!([{ "path": "plugin.py", "content": "print('v2')\n" }]);
    read_back["manifest"]["version"] = json!("0.2.0");
    read_back["replace"] = json!(true);
    let replaced = invoke(&kernel, SAVE_TOOL, read_back).await;
    assert!(!replaced.is_error, "{}", output_text(&replaced));
    assert_eq!(read("plugin.py"), "print('v2')\n");
    assert!(!plugin.join("lib").exists());
    let manifest: Value = serde_json::from_str(&read(MANIFEST_FILE)).unwrap();
    assert_eq!(manifest["version"], json!("0.2.0"));
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);

    let nodes = render(&kernel).await.nodes;
    assert!(matches!(
        &nodes[1],
        Node::Table { rows, .. } if rows == &[vec![
            "greeter".to_owned(),
            "python".to_owned(),
            "greeter.hello".to_owned(),
            "Greets people by name.".to_owned(),
        ]]
    ));
    assert_eq!(
        nodes[2],
        Node::Heading {
            text: "greeter".into()
        }
    );
    assert!(matches!(&nodes[3], Node::Markdown { text } if text.contains("id = \"greeter\"")));

    assert!(
        invoke(&kernel, READ_TOOL, json!({ "name": "missing" }))
            .await
            .is_error
    );
    assert!(
        invoke(&kernel, READ_TOOL, json!({ "name": "../x" }))
            .await
            .is_error
    );
}

#[tokio::test]
async fn a_directory_the_builder_did_not_write_is_never_replaced() {
    let dir = TempDir::new("foreign");
    std::fs::create_dir_all(dir.0.join("greeter")).unwrap();
    std::fs::write(dir.0.join("greeter/notes.txt"), "mine").unwrap();
    let kernel = Kernel::new();
    mount_builder(&kernel, &dir.0).await;
    let input = greeter_with(|i| i["replace"] = json!(true));
    let refused = invoke(&kernel, SAVE_TOOL, input).await;
    assert!(refused.is_error);
    assert!(output_text(&refused).contains("not written by plugin-builder"));
    assert_eq!(
        std::fs::read_to_string(dir.0.join("greeter/notes.txt")).unwrap(),
        "mine"
    );
    // It is not listed either.
    assert!(matches!(
        render(&kernel).await.nodes.as_slice(),
        [_, Node::Text { text }, Node::Heading { .. }, Node::Form { action, .. }]
            if text == "No personal plugins yet." && action == IMPORT_ACTION
    ));
}

#[tokio::test]
async fn a_node_plugin_gets_the_node_runtime() {
    let dir = TempDir::new("node");
    let kernel = Kernel::new();
    mount_builder(&kernel, &dir.0).await;
    let input = greeter_with(|i| {
        i["runtime"] = json!("node");
        i["files"] = json!([{ "path": "plugin.mjs", "content": "// hi\n" }]);
    });
    assert!(!invoke(&kernel, SAVE_TOOL, input).await.is_error);
    let plugin = dir.0.join("greeter");
    assert_eq!(
        std::fs::read_to_string(plugin.join("rah_plugin.mjs")).unwrap(),
        include_str!("runtime/rah_plugin.mjs")
    );
    let mount = std::fs::read_to_string(plugin.join(MOUNT_FILE)).unwrap();
    assert!(mount.contains("program = \"node\"") && mount.contains("plugin.mjs"));
}

async fn act(kernel: &Kernel, action: &str, input: Value) -> Result<ActionOutcome, UiError> {
    let all = kernel.handle().get_all(UI_VIEWS);
    let (_, views) = all.iter().find(|(id, _)| id == "plugin-builder").unwrap();
    views
        .act(VIEW, action, input, &ViewContext::default())
        .await
}

#[tokio::test]
async fn ticked_plugins_are_exported_to_a_bundle_that_imports_elsewhere() {
    let home = TempDir::new("export");
    let kernel = Kernel::new();
    mount_builder(&kernel, &home.0).await;
    assert!(!invoke(&kernel, SAVE_TOOL, greeter()).await.is_error);
    let other = greeter_with(|i| {
        i["name"] = json!("farewell");
        i["manifest"]["tools"][0]["name"] = json!("farewell.hello");
    });
    assert!(!invoke(&kernel, SAVE_TOOL, other).await.is_error);

    // The export form has a check box per plugin and a file field.
    let nodes = render(&kernel).await.nodes;
    let Some(Node::Form { fields, .. }) = nodes
        .iter()
        .find(|node| matches!(node, Node::Form { action, .. } if action == EXPORT_ACTION))
    else {
        panic!("no export form in {nodes:?}");
    };
    let names: Vec<(&str, FieldKind)> = fields.iter().map(|f| (f.name.as_str(), f.kind)).collect();
    assert_eq!(
        names,
        [
            ("plugin:farewell", FieldKind::Boolean),
            ("plugin:greeter", FieldKind::Boolean),
            ("file", FieldKind::Text),
        ]
    );

    // Nothing ticked is refused; only the ticked plugin is exported.
    assert!(matches!(
        act(
            &kernel,
            EXPORT_ACTION,
            json!({ "plugin:greeter": false, "file": "" })
        )
        .await,
        Err(UiError::InvalidInput(_))
    ));
    let exported = act(
        &kernel,
        EXPORT_ACTION,
        json!({ "plugin:greeter": true, "plugin:farewell": false, "file": "out/mine.json" }),
    )
    .await
    .unwrap();
    assert!(exported.message.unwrap().contains("Exported greeter to"));
    let bundle_path = home.0.join("out/mine.json");
    let bundle: Value =
        serde_json::from_str(&std::fs::read_to_string(&bundle_path).unwrap()).unwrap();
    assert_eq!(bundle["format"], json!("rah-personal-plugins"));
    assert_eq!(bundle["version"], json!(1));
    assert_eq!(bundle["plugins"].as_array().unwrap().len(), 1);
    assert_eq!(bundle["plugins"][0]["name"], json!("greeter"));
    // An existing file is never overwritten.
    assert!(matches!(
        act(&kernel, EXPORT_ACTION, json!({ "plugin:farewell": true, "file": "out/mine.json" })).await,
        Err(UiError::Failed(reason)) if reason.contains("exists")
    ));
    // Without a file name, it goes to `exports/`.
    let default = act(
        &kernel,
        EXPORT_ACTION,
        json!({ "plugin:farewell": true, "file": null }),
    )
    .await
    .unwrap();
    assert!(default.message.unwrap().contains("personal-plugins-"));
    assert_eq!(
        std::fs::read_dir(home.0.join("exports")).unwrap().count(),
        1
    );

    // Another machine imports it; nothing is mounted.
    let elsewhere = TempDir::new("import");
    let kernel2 = Kernel::new();
    mount_builder(&kernel2, &elsewhere.0).await;
    let file = bundle_path.display().to_string();
    let imported = act(
        &kernel2,
        IMPORT_ACTION,
        json!({ "file": file, "replace": false }),
    )
    .await
    .unwrap();
    assert!(imported.rerender);
    assert!(imported.message.unwrap().contains("Imported greeter"));
    assert_eq!(
        std::fs::read_to_string(elsewhere.0.join("greeter/plugin.py")).unwrap(),
        "print('hi')\n"
    );
    let mount = std::fs::read_to_string(elsewhere.0.join("greeter").join(MOUNT_FILE)).unwrap();
    assert!(
        mount.contains(
            &elsewhere
                .0
                .join("greeter")
                .display()
                .to_string()
                .replace('\\', "\\\\")
        )
    );
    // Again: skipped without `replace`, replaced with it.
    let again = act(
        &kernel2,
        IMPORT_ACTION,
        json!({ "file": file, "replace": false }),
    )
    .await
    .unwrap();
    assert!(!again.rerender && again.message.unwrap().contains("Skipped: greeter"));
    let replaced = act(
        &kernel2,
        IMPORT_ACTION,
        json!({ "file": file, "replace": true }),
    )
    .await
    .unwrap();
    assert!(replaced.message.unwrap().contains("Imported greeter"));

    // A bundle with one bad plugin imports none of them.
    let mut bad = bundle.clone();
    let mut broken = bundle["plugins"][0].clone();
    broken["name"] = json!("broken");
    broken["files"] = json!([{ "path": "../escape.py", "content": "" }]);
    let fine = greeter_with(|i| {
        i["name"] = json!("fine");
        i["manifest"]["tools"][0]["name"] = json!("fine.hello");
    });
    bad["plugins"] = json!([fine, broken]);
    let bad_path = elsewhere.0.join("bad.json");
    std::fs::write(&bad_path, bad.to_string()).unwrap();
    assert!(matches!(
        act(&kernel2, IMPORT_ACTION, json!({ "file": "bad.json" })).await,
        Err(UiError::InvalidInput(reason)) if reason.contains("`broken`")
    ));
    assert!(!elsewhere.0.join("fine").exists());
    // Not a bundle, or no file.
    std::fs::write(elsewhere.0.join("x.json"), "{}").unwrap();
    for input in [
        json!({ "file": "x.json" }),
        json!({ "file": " " }),
        json!({ "file": "missing.json" }),
    ] {
        assert!(matches!(
            act(&kernel2, IMPORT_ACTION, input).await,
            Err(UiError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        act(&kernel2, "delete", json!({})).await,
        Err(UiError::UnknownAction(_))
    ));
}

#[test]
fn the_configuration_is_checked() {
    let plugin = PluginBuilderPlugin::new("plugin-builder");
    let example: Value = serde_json::from_str(PluginBuilderPlugin::CONFIG_EXAMPLE).unwrap();
    assert!(plugin.check_config(&example).is_ok());
    assert!(plugin.check_config(&Value::Null).is_ok());
    for bad in [
        json!({ "x": 1 }),
        json!({ "python": " " }),
        json!({ "max_files": 0 }),
        json!({ "max_files": 1000 }),
        json!({ "max_file_bytes": 0 }),
        json!({ "max_file_bytes": 100_000_000 }),
    ] {
        assert!(plugin.check_config(&bad).is_err(), "{bad}");
    }
}
