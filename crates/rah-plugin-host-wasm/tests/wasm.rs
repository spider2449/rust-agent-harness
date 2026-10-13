//! The WASM host against a guest component written in the component model's
//! text format, so the tests need no WebAssembly compiler target.
//!
//! Guest tools, dispatched by name length: `wasm.echo` (9) returns its input;
//! `wasm.sessions` (13) calls `session.list`; `wasm.sandbox` (12) calls a
//! service the guest did not require; `wasm.store` (10) and `wasm.stored`
//! (11) call `storage.put` and `storage.get` with their input as parameters;
//! `wasm.spin-forever` (17) never returns. Any other name fails with
//! `invalid_input:`.

use std::{path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    CancellationToken, Kernel, MountOptions, Plugin, PluginError, PluginId, PluginManifest,
    PluginStatus, Scope, async_trait,
    session::{SESSION, SessionError, SessionLog},
    storage::{KvStore, MemoryStore, STORAGE},
    tools::{Invocation, ToolInvoker},
};
use rah_plugin_host_wasm::WasmPlugin;
use rah_protocol::{
    SessionEntry, SessionId, SessionInfo, SessionRecord, ToolCall, ToolCallId, ToolContent,
    ToolEffect, ToolInput, ToolName, ToolOutput,
};
use serde_json::{Value, json};

fn wat_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A guest component whose manifest is `manifest`.
fn guest(manifest: &Value) -> String {
    let manifest = manifest.to_string();
    let unknown = "invalid_input: unknown tool";
    format!(
        r#"(component
  (import "rah:plugin/host@0.1.0" (instance $host
    (export "call-service" (func (param "service" string) (param "method" string) (param "params" string) (result (result string (error string)))))
  ))
  (core module $libc
    (memory (export "memory") 17)
    (global $heap (mut i32) (i32.const 65536))
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (local $ptr i32)
      (local.set $ptr
        (i32.and
          (i32.add (global.get $heap) (i32.sub (local.get 2) (i32.const 1)))
          (i32.xor (i32.sub (local.get 2) (i32.const 1)) (i32.const -1))))
      (global.set $heap (i32.add (local.get $ptr) (local.get 3)))
      (local.get $ptr))
  )
  (core instance $libc (instantiate $libc))
  (core func $call_service (canon lower (func $host "call-service") (memory (core memory $libc "memory")) (realloc (core func $libc "realloc"))))
  (core module $main
    (import "libc" "memory" (memory 17))
    (import "host" "call-service" (func $call (param i32 i32 i32 i32 i32 i32 i32)))
    (data (i32.const 1024) "{manifest}")
    (data (i32.const 60000) "session")
    (data (i32.const 60008) "list")
    (data (i32.const 60016) "{{}}")
    (data (i32.const 60024) "sandbox")
    (data (i32.const 60032) "run")
    (data (i32.const 60040) "{unknown}")
    (data (i32.const 61000) "storage")
    (data (i32.const 61008) "put")
    (data (i32.const 61012) "get")
    (func (export "manifest") (result i32)
      (i32.store (i32.const 16) (i32.const 1024))
      (i32.store (i32.const 20) (i32.const {manifest_len}))
      (i32.const 16))
    (func (export "call-tool") (param $np i32) (param $nl i32) (param $ip i32) (param $il i32) (result i32)
      (if (i32.eq (local.get $nl) (i32.const 9))
        (then
          (i32.store8 (i32.const 32) (i32.const 0))
          (i32.store (i32.const 36) (local.get $ip))
          (i32.store (i32.const 40) (local.get $il))
          (return (i32.const 32))))
      (if (i32.eq (local.get $nl) (i32.const 13))
        (then
          (call $call (i32.const 60000) (i32.const 7) (i32.const 60008) (i32.const 4) (i32.const 60016) (i32.const 2) (i32.const 48))
          (return (i32.const 48))))
      (if (i32.eq (local.get $nl) (i32.const 12))
        (then
          (call $call (i32.const 60024) (i32.const 7) (i32.const 60032) (i32.const 3) (i32.const 60016) (i32.const 2) (i32.const 48))
          (return (i32.const 48))))
      (if (i32.eq (local.get $nl) (i32.const 10))
        (then
          (call $call (i32.const 61000) (i32.const 7) (i32.const 61008) (i32.const 3) (local.get $ip) (local.get $il) (i32.const 48))
          (return (i32.const 48))))
      (if (i32.eq (local.get $nl) (i32.const 11))
        (then
          (call $call (i32.const 61000) (i32.const 7) (i32.const 61012) (i32.const 3) (local.get $ip) (local.get $il) (i32.const 48))
          (return (i32.const 48))))
      (if (i32.eq (local.get $nl) (i32.const 17))
        (then (loop $forever (br $forever))))
      (i32.store8 (i32.const 32) (i32.const 1))
      (i32.store (i32.const 36) (i32.const 60040))
      (i32.store (i32.const 40) (i32.const {unknown_len}))
      (i32.const 32))
  )
  (core instance $main (instantiate $main
    (with "libc" (instance $libc))
    (with "host" (instance (export "call-service" (func $call_service))))
  ))
  (func $manifest (result string)
    (canon lift (core func $main "manifest") (memory (core memory $libc "memory")) (realloc (core func $libc "realloc"))))
  (export "manifest" (func $manifest))
  (func $call_tool (param "name" string) (param "input" string) (result (result string (error string)))
    (canon lift (core func $main "call-tool") (memory (core memory $libc "memory")) (realloc (core func $libc "realloc"))))
  (export "call-tool" (func $call_tool))
)"#,
        manifest = wat_string(&manifest),
        manifest_len = manifest.len(),
        unknown = unknown,
        unknown_len = unknown.len(),
    )
}

fn tool(name: &str, permission: &str) -> Value {
    json!({
        "name": name,
        "description": format!("fixture {name}"),
        "input_schema": { "type": "object" },
        "permission": permission
    })
}

fn manifest(requires: &[&str]) -> Value {
    json!({
        "version": "0.1.0",
        "requires": requires,
        "tools": [
            tool("wasm.echo", "none"),
            tool("wasm.sessions", "read"),
            tool("wasm.sandbox", "read"),
            tool("wasm.spin-forever", "none"),
            tool("wasm.x", "none"),
            tool("wasm.store", "write"),
            tool("wasm.stored", "read"),
        ]
    })
}

struct Fixture(PathBuf);

impl Fixture {
    fn new(name: &str, manifest: &Value) -> Self {
        let path = std::env::temp_dir().join(format!("rah-wasm-{name}-{}.wat", std::process::id()));
        std::fs::write(&path, guest(manifest)).unwrap();
        Self(path)
    }

    fn config(&self) -> Value {
        json!({ "path": self.0, "fuel": 5_000_000 })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// One session with one record.
struct Sessions(SessionId);

#[async_trait]
impl SessionLog for Sessions {
    async fn create(&self) -> Result<SessionInfo, SessionError> {
        unreachable!()
    }

    async fn append(&self, _: &SessionId, _: SessionEntry) -> Result<u64, SessionError> {
        unreachable!()
    }

    async fn read(&self, _: &SessionId) -> Result<Vec<SessionRecord>, SessionError> {
        Ok(Vec::new())
    }

    async fn fork(&self, _: &SessionId, _: u64) -> Result<SessionInfo, SessionError> {
        unreachable!()
    }

    async fn list(&self) -> Result<Vec<SessionInfo>, SessionError> {
        Ok(vec![SessionInfo {
            id: self.0.clone(),
            parent: None,
            created: 7,
            title: None,
            updated: 7,
            preview: None,
            workspace: None,
        }])
    }
}

struct SessionPlugin(PluginManifest, Arc<Sessions>);

#[async_trait]
impl Plugin for SessionPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(SESSION, self.1.clone() as Arc<dyn SessionLog>)?;
        Ok(())
    }
}

async fn call(kernel: &Kernel, name: &str, input: Value) -> (ToolOutput, ToolEffect) {
    call_with(kernel, name, input, CancellationToken::new()).await
}

async fn call_with(
    kernel: &Kernel,
    name: &str,
    input: Value,
    cancel: CancellationToken,
) -> (ToolOutput, ToolEffect) {
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(name),
                    input: ToolInput(input),
                },
                cancel,
            },
            || {},
        )
        .await;
    (outcome.output, outcome.effect)
}

fn text(output: &ToolOutput) -> String {
    match &output.content[0] {
        ToolContent::Text(text) => text.clone(),
        ToolContent::Json(value) => value.to_string(),
        other => panic!("unexpected content: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_component_provides_tools_and_reaches_only_required_services() {
    let fixture = Fixture::new("main", &manifest(&["session"]));
    let plugin = WasmPlugin::load(PluginId::new("wasm"), &fixture.config())
        .await
        .unwrap();
    assert_eq!(plugin.described().tools.len(), 7);
    assert_eq!(plugin.manifest().requires, ["session".into()]);

    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(plugin),
            MountOptions::with_config(fixture.config()),
        )
        .await
        .unwrap();
    assert!(matches!(status, PluginStatus::Pending { .. }), "{status:?}");
    let session = SessionId::new();
    kernel
        .mount(
            Arc::new(SessionPlugin(
                PluginManifest::new("sessions", "1").provides(["session"]),
                Arc::new(Sessions(session.clone())),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        kernel.status(&PluginId::new("wasm")),
        Some(PluginStatus::Active)
    );

    let (output, effect) = call(&kernel, "wasm.echo", json!({ "text": "hi" })).await;
    assert_eq!(text(&output), r#"{"text":"hi"}"#);
    assert_eq!(effect, ToolEffect::None);

    let (output, _) = call(&kernel, "wasm.sessions", json!({})).await;
    let listed: Value = serde_json::from_str(&text(&output)).unwrap();
    assert_eq!(listed[0]["id"], session.to_string());

    let (output, _) = call(&kernel, "wasm.sandbox", json!({})).await;
    assert!(output.is_error);
    assert!(
        text(&output).contains("not in this plugin's requires"),
        "{}",
        text(&output)
    );

    let (output, effect) = call(&kernel, "wasm.x", json!({})).await;
    assert!(
        text(&output).starts_with("invalid input: unknown tool"),
        "{}",
        text(&output)
    );
    assert_eq!(effect, ToolEffect::None);

    let (output, _) = call(&kernel, "wasm.spin-forever", json!({})).await;
    assert!(output.is_error);
    assert!(text(&output).contains("trapped"), "{}", text(&output));
    // The guest is fresh on every call; it still works after a trap.
    let (output, _) = call(&kernel, "wasm.echo", json!({ "again": true })).await;
    assert_eq!(text(&output), r#"{"again":true}"#);
}

struct StoragePlugin(PluginManifest, Arc<MemoryStore>);

#[async_trait]
impl Plugin for StoragePlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(STORAGE, self.1.clone() as Arc<dyn KvStore>)?;
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_component_keeps_state_in_the_namespace_of_its_id() {
    let fixture = Fixture::new("storage", &manifest(&["storage"]));
    let plugin = WasmPlugin::load(PluginId::new("counter"), &fixture.config())
        .await
        .unwrap();
    let kernel = Kernel::new();
    let store = Arc::new(MemoryStore::new());
    store.put("other", "k", b"\"theirs\"").await.unwrap();
    kernel
        .mount(
            Arc::new(StoragePlugin(
                PluginManifest::new("storage", "1").provides(["storage"]),
                store.clone(),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let status = kernel
        .mount(
            Arc::new(plugin),
            MountOptions::with_config(fixture.config()),
        )
        .await
        .unwrap();
    assert_eq!(status, PluginStatus::Active);

    let (output, _) = call(
        &kernel,
        "wasm.store",
        json!({ "key": "k", "value": [1, 2], "namespace": "other" }),
    )
    .await;
    assert_eq!(text(&output), "{}");
    assert_eq!(
        store.get("counter", "k").await.unwrap(),
        Some(b"[1,2]".to_vec())
    );
    assert_eq!(
        store.get("other", "k").await.unwrap(),
        Some(b"\"theirs\"".to_vec())
    );
    let (output, _) = call(&kernel, "wasm.stored", json!({ "key": "k" })).await;
    assert_eq!(text(&output), r#"{"value":[1,2]}"#);

    let (output, _) = call(&kernel, "wasm.stored", json!({ "key": "a/b" })).await;
    assert!(output.is_error);
    assert!(
        text(&output).contains("invalid storage name"),
        "{}",
        text(&output)
    );
    // `session` is not in this guest's requires.
    let (output, _) = call(&kernel, "wasm.sessions", json!({})).await;
    assert!(text(&output).contains("not in this plugin's requires"));
}

/// With one blocking thread, a guest that kept running after its caller
/// cancelled would block every later call.
#[test]
fn cancelling_a_call_interrupts_its_guest() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.block_on(cancel_and_call_again())
    }));
    // A guest that was not interrupted still holds the blocking thread;
    // do not wait for it, or a failure would hang instead of failing.
    runtime.shutdown_background();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

async fn cancel_and_call_again() {
    let fixture = Fixture::new("cancel", &manifest(&[]));
    // Enough fuel to spin for far longer than the test may take.
    let config = json!({ "path": fixture.0, "fuel": u64::MAX / 2 });
    let plugin = WasmPlugin::load(PluginId::new("wasm"), &config)
        .await
        .unwrap();
    let kernel = Kernel::new();
    kernel
        .mount(Arc::new(plugin), MountOptions::with_config(config))
        .await
        .unwrap();

    let cancel = CancellationToken::new();
    let spinning = tokio::spawn({
        let cancel = cancel.clone();
        let kernel = kernel.handle();
        async move {
            let invoker = ToolInvoker::new(kernel);
            invoker
                .invoke(
                    Invocation {
                        session: SessionId::new(),
                        call: ToolCall {
                            id: ToolCallId::new(),
                            name: ToolName::new("wasm.spin-forever"),
                            input: ToolInput(json!({})),
                        },
                        cancel,
                    },
                    || {},
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !spinning.is_finished(),
        "the guest should still be spinning"
    );
    cancel.cancel();
    let outcome = tokio::time::timeout(Duration::from_secs(5), spinning)
        .await
        .expect("the cancelled call returns")
        .unwrap();
    assert!(outcome.output.is_error);
    assert_eq!(outcome.effect, ToolEffect::None);

    // The only blocking thread is free again.
    let (output, _) = tokio::time::timeout(
        Duration::from_secs(10),
        call_with(
            &kernel,
            "wasm.echo",
            json!({ "after": "cancel" }),
            CancellationToken::new(),
        ),
    )
    .await
    .expect("the interrupted guest released its thread");
    assert_eq!(text(&output), r#"{"after":"cancel"}"#);
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_components_and_manifests_fail_to_load() {
    let fixture = Fixture::new("bad-requires", &manifest(&["sandbox"]));
    let error = WasmPlugin::load(PluginId::new("wasm"), &fixture.config())
        .await
        .err()
        .unwrap();
    assert!(error.contains("cannot depend on `sandbox`"), "{error}");

    let mut no_tools = manifest(&[]);
    no_tools["tools"] = json!([]);
    let fixture = Fixture::new("no-tools", &no_tools);
    assert!(
        WasmPlugin::load(PluginId::new("wasm"), &fixture.config())
            .await
            .is_err()
    );

    let broken = std::env::temp_dir().join(format!("rah-wasm-broken-{}.wat", std::process::id()));
    std::fs::write(&broken, "(component (export \"nothing\"))").unwrap();
    let error = WasmPlugin::load(PluginId::new("wasm"), &json!({ "path": broken }))
        .await
        .err()
        .unwrap();
    let _ = std::fs::remove_file(&broken);
    assert!(error.contains("cannot load"), "{error}");
    assert!(
        WasmPlugin::load(PluginId::new("wasm"), &json!({ "path": "missing.wasm" }))
            .await
            .is_err()
    );
}
