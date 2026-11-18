//! Explicit local-toolchain validation; no model or network is used.
mod common;

use rah_api::{
    Kernel, MountOptions,
    hooks::{ToolAfterInvoke, ToolCompletion, ToolRequestContext},
};
use rah_protocol::*;
use serde_json::json;
use std::{path::Path, sync::Arc};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires the prepared LSP bundle and a local Rust toolchain"]
async fn rust_first_write_and_correction_through_the_hook() {
    let dir = common::TempDir::new("rust-lsp");
    let root = dir.workspace();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='lsp-fixture'\nversion='0.1.0'\nedition='2021'\n",
    )
    .unwrap();
    let server = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.rah/lsp-runtime/rust-analyzer.exe")
        .canonicalize()
        .unwrap();
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(rah_plugin_sandbox_process::SandboxProcessPlugin::new(
                "sandbox",
            )),
            MountOptions::with_config(json!({"root":root})),
        )
        .await
        .unwrap();
    kernel.mount(Arc::new(rah_plugin_lsp_diagnostics::LspDiagnosticsPlugin::new("lsp")), MountOptions::with_config(json!({
        "root":root,"defaults":false,"wait_ms":30000,"start_timeout_ms":30000,
        "servers":{"rust":{"command":[server],"extensions":[".rs"],"env":{"CARGO_NET_OFFLINE":"true"},
        "initialization":{"cargo":{"buildScripts":{"enable":false}},"procMacro":{"enable":false}}}}
    }))).await.unwrap();
    for (source, bad) in [
        ("fn main() { let _: u32 = \"wrong\"; }\n", true),
        ("fn main() { let _: u32 = 1; }\n", false),
    ] {
        std::fs::write(root.join("src/main.rs"), source).unwrap();
        let completion = kernel
            .handle()
            .waterfall::<ToolAfterInvoke>(ToolCompletion {
                request: ToolRequestContext {
                    session: SessionId::new(),
                    call: ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new("repo.create-file"),
                        input: ToolInput(json!({"path":"src/main.rs"})),
                    },
                    definition: ToolDefinition {
                        name: ToolName::new("repo.create-file"),
                        description: String::new(),
                        input_schema: json!({}),
                        permission: PermissionLevel::Write,
                    },
                },
                output: ToolOutput {
                    content: vec![],
                    is_error: false,
                },
                effect: ToolEffect::Completed,
            })
            .await
            .unwrap();
        let output = format!("{:?}", completion.output.content);
        eprintln!("bad={bad}: {output}");
        if bad {
            assert!(
                output.contains("mismatched types") || output.contains("expected u32"),
                "{output}"
            );
        } else {
            assert!(completion.output.content.is_empty(), "{output}");
        }
    }
    kernel
        .unmount(&rah_api::PluginId::new("lsp"))
        .await
        .unwrap();
    kernel
        .unmount(&rah_api::PluginId::new("sandbox"))
        .await
        .unwrap();
}
