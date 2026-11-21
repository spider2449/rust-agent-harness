//! Exercises the CBM adapter with an offline backend and the real tool boundary.

mod common;

use std::{fs, path::Path};

use common::{TempDir, kernel, new_session, reply, turn};
use rah_api::{admin::ADMIN, tools::ToolInvoker};
use rah_cli::Distribution;
use serde_json::json;

#[tokio::test]
#[ignore = "requires Node.js on PATH"]
async fn readonly_cbm_tools_follow_the_workspace() {
    let dir = TempDir::new("cbm-mcp");
    let root = dir.workspace();
    let other = dir.0.join("other");
    fs::create_dir(&other).unwrap();
    let adapter = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/mcp/cbm-readonly.mjs");
    let fixture = dir.0.join("cbm-fixture.mjs");
    fs::write(
        &fixture,
        format!(
            r#"import {{ pathToFileURL }} from 'node:url';
const {{ serve }} = await import(pathToFileURL({}));
serve(process.stdin, process.stdout, async (name, args) => ({{
  content: [{{ type: 'text', text: JSON.stringify({{ name, args, cwd: process.cwd() }}) }}]
}}));"#,
            json!(adapter)
        ),
    )
    .unwrap();
    let mut distribution: Distribution =
        toml::from_str(include_str!("../../../examples/mcp/cbm.toml")).unwrap();
    distribution
        .plugin
        .retain(|spec| spec.kind != "frontend-cli");
    distribution.set_config("sandbox-process", "root", json!(root));
    distribution.set_config("session-jsonl", "dir", json!(dir.sessions()));
    distribution.set_config("tools-mcp", "args", json!([fixture]));
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let definitions = ToolInvoker::new(handle.clone()).definitions();
    let expected = [
        ("search_graph", json!({"query":"method"})),
        ("trace_path", json!({"function_name":"method"})),
        (
            "get_code_snippet",
            json!({"qualified_name":"project.method"}),
        ),
        ("query_graph", json!({"query":"MATCH (n) RETURN n LIMIT 1"})),
    ];
    assert_eq!(definitions.len(), expected.len());
    for (name, args) in expected {
        let full = format!("mcp.cbm.{name}");
        assert!(definitions.iter().any(|tool| {
            tool.name.as_str() == full && tool.permission == rah_protocol::PermissionLevel::Read
        }));
        let events = turn(
            &handle,
            &new_session(&handle).await,
            &format!("/call {full} {args}"),
        )
        .await;
        assert!(reply(&events).contains(name), "{events:?}");
        assert!(reply(&events).contains("rah-workspace"), "{events:?}");
    }
    let invoke_search = r#"/call mcp.cbm.search_graph {"query":"method","project":"other"}"#;
    let events = turn(&handle, &new_session(&handle).await, invoke_search).await;
    assert!(reply(&events).contains("Unknown CBM project"));
    handle
        .get(ADMIN)
        .unwrap()
        .set_workspace(&other)
        .await
        .unwrap();
    let events = turn(
        &handle,
        &new_session(&handle).await,
        r#"/call mcp.cbm.search_graph {"query":"method"}"#,
    )
    .await;
    assert!(reply(&events).contains("other"), "{events:?}");
    kernel.shutdown().await.unwrap();
}
