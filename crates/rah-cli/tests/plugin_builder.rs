//! `plugin-builder` end to end: the model saves a personal plugin with
//! `plugins.save`, and a distribution that adds the saved `mount.toml` runs
//! it through the process host with the runtime library, for both runtimes.

mod common;

use std::process::Command;

use common::{TempDir, config_file, from_file, new_session, reply, turn};
use rah_api::{
    Kernel,
    ui::{UI_VIEWS, UiError, ViewContext},
};
use rah_cli::{Distribution, boot};
use serde_json::{Value, json};

/// The first of `programs` on PATH whose `--version` output starts with
/// `version` (a Windows Store alias succeeds without printing one).
fn program(programs: &[&'static str], version: &str) -> Option<&'static str> {
    programs.iter().copied().find(|program| {
        Command::new(program)
            .arg("--version")
            .output()
            .is_ok_and(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).starts_with(version)
            })
    })
}

fn forward(path: &std::path::Path) -> String {
    path.display().to_string().replace('\\', "/")
}

const NODE_PLUGIN: &str = r#"import { Plugin, InvalidInput } from "./rah_plugin.mjs";

const plugin = new Plugin(import.meta.url);

async function count(ctx) {
  const stored = await ctx.callHost("storage", "get", { key: "count" });
  return stored.value ?? 0;
}

plugin.tool("greeter.hello", async (args) => {
  if (typeof args.name !== "string") throw new InvalidInput("`name` must be a string");
  return `Hello, ${args.name}!`;
});

plugin.tool("greeter.count", async (args, ctx) => {
  const next = (await count(ctx)) + 1;
  await ctx.callHost("storage", "put", { key: "count", value: next });
  ctx.viewChanged("board");
  return { count: next };
});

plugin.view("board", async (ctx) => [{ type: "text", text: `count ${await count(ctx)}` }]);

plugin.action("board", "reset", async (input, ctx) => {
  await ctx.callHost("storage", "put", { key: "count", value: 0 });
  return { message: "reset", rerender: true };
});

plugin.run();
"#;

const PYTHON_PLUGIN: &str = r#"from rah_plugin import Plugin, InvalidInput

plugin = Plugin(__file__)


def count(ctx):
    return ctx.call_host("storage", "get", {"key": "count"}).get("value") or 0


@plugin.tool("greeter.hello")
def hello(args, ctx):
    if not isinstance(args.get("name"), str):
        raise InvalidInput("`name` must be a string")
    return f"Hello, {args['name']}!"


@plugin.tool("greeter.count")
def bump(args, ctx):
    value = count(ctx) + 1
    ctx.call_host("storage", "put", {"key": "count", "value": value})
    ctx.view_changed("board")
    return {"count": value}


@plugin.view("board")
def board(ctx):
    return [{"type": "text", "text": f"count {count(ctx)}"}]


@plugin.action("board", "reset")
def reset(input, ctx):
    ctx.call_host("storage", "put", {"key": "count", "value": 0})
    return {"message": "reset", "rerender": True}


plugin.run()
"#;

fn save_input(runtime: &str) -> Value {
    let (entry, content) = match runtime {
        "node" => ("plugin.mjs", NODE_PLUGIN),
        _ => ("plugin.py", PYTHON_PLUGIN),
    };
    let tool = |name: &str, description: &str| {
        json!({
            "name": name,
            "description": description,
            "input_schema": { "type": "object" },
            "permission": "none"
        })
    };
    json!({
        "name": "greeter",
        "description": "Greets people and counts calls.",
        "runtime": runtime,
        "manifest": {
            "version": "0.1.0",
            "requires": ["storage"],
            "tools": [
                tool("greeter.hello", "Greets someone by name."),
                tool("greeter.count", "Counts its calls.")
            ],
            "views": [{ "id": "board", "title": "Greeter" }]
        },
        "files": [{ "path": entry, "content": content }]
    })
}

/// The distribution the model works in: it may save plugins.
fn builder_distribution(dir: &TempDir, program: &str) -> Distribution {
    Distribution::parse(&format!(
        r#"
[[plugin]]
use = "policy-authority"
config = {{ allow = ["none", "read", "write"], ask = ["execute"], tools = {{ "plugins.guide" = "none", "plugins.save" = "write" }} }}

[[plugin]]
use = "session-jsonl"
config = {{ dir = "{sessions}" }}

[[plugin]]
use = "model-mock"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "plugin-builder"
config = {{ dir = "{plugins}", python = "{program}", node = "{program}" }}
"#,
        sessions = forward(&dir.sessions()),
        plugins = forward(&dir.0.join("plugins")),
    ))
    .unwrap()
}

/// The user's distribution after adding the saved `mount.toml`.
fn user_distribution(dir: &TempDir, mount: &str) -> Distribution {
    Distribution::parse(&format!(
        r#"
[[plugin]]
use = "policy-authority"
config = {{ tools = {{ "greeter.*" = "none" }} }}

[[plugin]]
use = "session-jsonl"
config = {{ dir = "{sessions}" }}

[[plugin]]
use = "storage-fs"
config = {{ dir = "{storage}" }}

[[plugin]]
use = "model-mock"

[[plugin]]
use = "loop-react"

{mount}
"#,
        sessions = forward(&dir.sessions()),
        storage = forward(&dir.storage()),
    ))
    .unwrap()
}

async fn saved_plugin_runs(runtime: &str, program: &str) {
    let dir = TempDir::new(&format!("plugin-builder-{runtime}"));

    // The model reads the rules and saves the plugin.
    let kernel = Kernel::new();
    boot(&kernel, &builder_distribution(&dir, program))
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let guide = reply(&turn(&handle, &session, "/call plugins.guide {}").await);
    assert!(guide.contains("Writing a personal RAH plugin"), "{guide}");
    let input = format!("/call plugins.save {}", save_input(runtime));
    let saved = reply(&turn(&handle, &session, &input).await);
    assert!(saved.contains("Saved plugin `greeter`"), "{saved}");
    kernel.shutdown().await.unwrap();

    // The user mounts it with the saved entry.
    let mount = std::fs::read_to_string(dir.0.join("plugins/greeter/mount.toml")).unwrap();
    let kernel = Kernel::new();
    boot(&kernel, &user_distribution(&dir, &mount))
        .await
        .unwrap_or_else(|error| panic!("{runtime}: {error}"));
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    let hello = reply(&turn(&handle, &session, r#"/call greeter.hello {"name":"Ann"}"#).await);
    assert!(hello.contains("Hello, Ann!"), "{hello}");
    let invalid = reply(&turn(&handle, &session, r#"/call greeter.hello {"name":5}"#).await);
    assert!(
        invalid.contains("invalid input: `name` must be a string"),
        "{invalid}"
    );
    reply(&turn(&handle, &session, "/call greeter.count {}").await);
    let counted = reply(&turn(&handle, &session, "/call greeter.count {}").await);
    assert!(counted.contains(r#""count":2"#), "{counted}");

    let all = handle.get_all(UI_VIEWS);
    let (instance, views) = all.iter().find(|(id, _)| id == "greeter").unwrap();
    assert_eq!(instance, "greeter");
    assert_eq!(views.views()[0].title, "Greeter");
    let cx = ViewContext::default();
    let text = |view: &rah_api::ui::View| serde_json::to_string(view).unwrap();
    assert!(text(&views.render("board", &cx).await.unwrap()).contains("count 2"));
    let reset = views.act("board", "reset", Value::Null, &cx).await.unwrap();
    assert!(reset.rerender);
    assert!(text(&views.render("board", &cx).await.unwrap()).contains("count 0"));
    assert!(matches!(
        views.act("board", "nope", Value::Null, &cx).await,
        Err(UiError::UnknownAction(_))
    ));
    kernel.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_saved_node_plugin_runs_once_the_user_mounts_it() {
    let Some(node) = program(&["node"], "v") else {
        eprintln!("skipping: no Node.js on PATH");
        return;
    };
    saved_plugin_runs("node", node).await;
}

#[tokio::test]
async fn a_saved_python_plugin_runs_once_the_user_mounts_it() {
    let Some(python) = program(&["python3", "python"], "Python 3") else {
        eprintln!("skipping: no Python 3 interpreter on PATH");
        return;
    };
    saved_plugin_runs("python", python).await;
}

#[tokio::test]
async fn saving_needs_the_policy() {
    let dir = TempDir::new("plugin-builder-policy");
    let mut distribution = builder_distribution(&dir, "node");
    // Without an approver, the default policy asks before a write and so
    // refuses it.
    distribution.set_config("policy-authority", "allow", json!(["none", "read"]));
    distribution.set_config("policy-authority", "ask", json!(["write", "execute"]));
    let kernel = Kernel::new();
    boot(&kernel, &distribution).await.unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let input = format!("/call plugins.save {}", save_input("node"));
    let refused = reply(&turn(&handle, &session, &input).await);
    assert!(!refused.contains("Saved plugin"), "{refused}");
    assert!(!dir.0.join("plugins/greeter").exists());
    kernel.shutdown().await.unwrap();
}

/// Runs the action `action` of the "My plugins" view.
async fn manage(kernel: &Kernel, action: &str, input: Value) -> String {
    let all = kernel.handle().get_all(UI_VIEWS);
    let (_, views) = all.iter().find(|(id, _)| id == "plugin-builder").unwrap();
    views
        .act("plugins", action, input, &ViewContext::default())
        .await
        .unwrap_or_else(|error| panic!("{action}: {error}"))
        .message
        .unwrap_or_default()
}

/// ADR 0073: the user enables, changes, disables and deletes a saved plugin
/// from the view, against the real `admin` service and configuration file.
#[tokio::test]
async fn a_saved_plugin_is_managed_from_its_panel() {
    let (runtime, program) = match (
        program(&["python3", "python"], "Python 3"),
        program(&["node"], "v"),
    ) {
        (Some(python), _) => ("python", python),
        (None, Some(node)) => ("node", node),
        (None, None) => {
            eprintln!("skipping: no Python 3 or Node.js on PATH");
            return;
        }
    };
    let dir = TempDir::new("plugin-builder-panel");
    let file = config_file(&dir);
    std::fs::write(
        &file,
        "model = \"mock\"\npermissions = \"full\"\nfeatures = [\"plugin-builder\"]\n",
    )
    .unwrap();
    let start = || {
        let mut distribution = from_file(&dir);
        for key in ["python", "node"] {
            distribution.set_config("plugin-builder", key, json!(program));
        }
        distribution
    };
    let kernel = Kernel::new();
    boot(&kernel, &start()).await.unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let hello = r#"/call greeter.hello {"name":"Ann"}"#;

    // Saved, it does not run and nothing in the file names it.
    let input = format!("/call plugins.save {}", save_input(runtime));
    let saved = reply(&turn(&handle, &session, &input).await);
    assert!(saved.contains("It is not enabled"), "{saved}");
    assert!(!reply(&turn(&handle, &session, hello).await).contains("Hello, Ann!"));
    let text = || std::fs::read_to_string(&file).unwrap();
    assert!(!text().contains("greeter"), "{}", text());

    // Enabled, it runs at once and the file keeps it with its assignments.
    let enabled = manage(&kernel, "enable:greeter", json!({ "read": true })).await;
    assert!(enabled.starts_with("Enabled `greeter`."), "{enabled}");
    assert!(!enabled.contains("not kept"), "{enabled}");
    let greeted = reply(&turn(&handle, &session, hello).await);
    assert!(greeted.contains("Hello, Ann!"), "{greeted}");
    let kept = text();
    assert!(kept.contains("use = \"process\""), "{kept}");
    assert!(kept.contains("id = \"greeter\""), "{kept}");
    assert!(kept.contains("\"greeter.hello\" = \"none\""), "{kept}");

    // What the file says is what the next start runs.
    kernel.shutdown().await.unwrap();
    let kernel = Kernel::new();
    boot(&kernel, &start())
        .await
        .unwrap_or_else(|error| panic!("the kept file does not start: {error}"));
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    assert!(reply(&turn(&handle, &session, hello).await).contains("Hello, Ann!"));

    // The model changes it while it runs; the earlier save answers until
    // the user reloads.
    let mut changed = save_input(runtime);
    let content = changed["files"][0]["content"].as_str().unwrap();
    changed["files"][0]["content"] = json!(content.replace("Hello, ", "Good day, "));
    changed["replace"] = json!(true);
    let saved = reply(&turn(&handle, &session, &format!("/call plugins.save {changed}")).await);
    assert!(saved.contains("still the earlier save"), "{saved}");
    assert!(reply(&turn(&handle, &session, hello).await).contains("Hello, Ann!"));
    let reloaded = manage(&kernel, "reload:greeter", json!({ "read": true })).await;
    assert!(reloaded.starts_with("Reloaded `greeter`."), "{reloaded}");
    let greeted = reply(&turn(&handle, &session, hello).await);
    assert!(greeted.contains("Good day, Ann!"), "{greeted}");

    // Disabled, its tools are gone and the file no longer names it.
    let disabled = manage(&kernel, "disable:greeter", Value::Null).await;
    assert!(disabled.starts_with("Disabled `greeter`"), "{disabled}");
    assert!(!disabled.contains("by hand"), "{disabled}");
    assert!(!reply(&turn(&handle, &session, hello).await).contains("Ann!"));
    assert!(!text().contains("greeter"), "{}", text());
    assert!(dir.0.join("plugins/greeter").is_dir());

    // Enabled again and deleted: it is disabled, copied and removed.
    manage(&kernel, "enable:greeter", json!({ "read": true })).await;
    let deleted = manage(&kernel, "delete:greeter", json!({ "confirm": "greeter" })).await;
    assert!(deleted.starts_with("Deleted `greeter`"), "{deleted}");
    assert!(deleted.contains("A copy is in"), "{deleted}");
    assert!(!dir.0.join("plugins/greeter").exists());
    assert!(!text().contains("greeter"), "{}", text());
    assert_eq!(
        std::fs::read_dir(dir.0.join("plugins/backups"))
            .unwrap()
            .count(),
        1
    );
    kernel.shutdown().await.unwrap();
}
