//! The Web UI driven in a real headless browser over the Chrome DevTools
//! Protocol: the embedded HTML, JavaScript and CSS under the real
//! Content-Security-Policy, against an in-process `rah` with the mock model.
//!
//! The test runs only when `RAH_TEST_BROWSER` names a Chromium-based browser
//! (Chrome, Chromium or Edge); otherwise it is skipped. It needs no network
//! access: the browser loads only the loopback server.

mod common;

use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use common::{TempDir, from_file};
use futures::{SinkExt, StreamExt};
use rah_api::{Kernel, host::FrontendListening};
use rah_cli::{Distribution, boot};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

/// How long a page condition may take. Generous: the first run after a
/// build is much slower on Windows.
const TIMEOUT: Duration = Duration::from_secs(30);
/// How long a DevTools command may take. The first page load after a build
/// took over 20 seconds on Windows (likely a scan of the new test binary).
const REPLY_TIMEOUT: Duration = Duration::from_secs(90);

/// One browser test at a time: several headless browsers and kernels at once
/// slowed a fresh build's first run on Windows past the timeouts.
static ONE_BROWSER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A headless browser process with its own profile; killed on drop.
struct Browser {
    child: Child,
    port: u16,
    profile: TempDir,
}

impl Browser {
    fn launch(program: PathBuf, name: &str) -> Self {
        let profile = TempDir::new(&format!("{name}-profile"));
        let mut command = Command::new(program);
        command
            .arg("--headless=new")
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", profile.0.display()))
            .args([
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-gpu",
                "--disable-extensions",
                "--disable-background-networking",
                "--disable-component-update",
                "--disable-sync",
                "--window-size=1280,900",
            ]);
        // CI containers often lack the user namespaces Chrome's sandbox needs.
        // The browser only loads the loopback test server.
        if cfg!(target_os = "linux") {
            command.arg("--no-sandbox");
        }
        let child = command
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("RAH_TEST_BROWSER starts");
        // With port 0 the browser writes the chosen port to this file.
        let active = profile.0.join("DevToolsActivePort");
        let deadline = Instant::now() + REPLY_TIMEOUT;
        let port = loop {
            if let Some(port) = std::fs::read_to_string(&active)
                .ok()
                .and_then(|text| text.lines().next()?.trim().parse().ok())
            {
                break port;
            }
            assert!(
                Instant::now() < deadline,
                "the browser never opened DevTools"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        Self {
            child,
            port,
            profile,
        }
    }

    /// Where downloads go: inside the temporary profile, never the user's
    /// download folder.
    fn downloads(&self) -> PathBuf {
        self.profile.0.join("downloads")
    }

    async fn page(&self) -> Page {
        let targets: Value = reqwest::get(format!("http://127.0.0.1:{}/json/list", self.port))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let url = targets
            .as_array()
            .unwrap()
            .iter()
            .find(|target| target["type"] == "page")
            .and_then(|target| target["webSocketDebuggerUrl"].as_str())
            .expect("the browser has a page")
            .to_owned();
        let socket = tokio_tungstenite::connect_async(url).await.unwrap().0;
        let mut page = Page {
            socket,
            next: 0,
            events: Vec::new(),
        };
        for domain in ["Page", "Runtime", "Log"] {
            page.call(&format!("{domain}.enable"), json!({})).await;
        }
        page.call(
            "Browser.setDownloadBehavior",
            json!({ "behavior": "allow", "downloadPath": self.downloads().display().to_string() }),
        )
        .await;
        page.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": HELPERS }),
        )
        .await;
        page
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One page target: commands in order, events kept for later checks.
struct Page {
    socket: Socket,
    next: u64,
    events: Vec<Value>,
}

impl Page {
    async fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        let message = json!({ "id": id, "method": method, "params": params });
        self.socket
            .send(Message::Text(message.to_string().into()))
            .await
            .unwrap();
        loop {
            let message = tokio::time::timeout(REPLY_TIMEOUT, self.socket.next())
                .await
                .unwrap_or_else(|_| panic!("no reply to {method}; events: {:#?}", self.events))
                .expect("the DevTools connection stays open")
                .unwrap();
            let Message::Text(text) = message else {
                continue;
            };
            let message: Value = serde_json::from_str(&text).unwrap();
            if message["id"] == id {
                assert!(message["error"].is_null(), "{method}: {message}");
                return message["result"].clone();
            }
            if message["method"].is_string() {
                self.events.push(message);
            }
        }
    }

    /// Evaluates `expression` in the page (awaiting a promise) and returns
    /// its JSON value.
    async fn eval(&mut self, expression: &str) -> Value {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({ "expression": expression, "awaitPromise": true, "returnByValue": true }),
            )
            .await;
        assert!(
            result["exceptionDetails"].is_null(),
            "`{expression}` threw: {}",
            result["exceptionDetails"]
        );
        result["result"]["value"].clone()
    }

    /// Polls `expression` until it is truthy; a throw counts as not yet.
    async fn wait(&mut self, what: &str, expression: &str) -> Value {
        let deadline = Instant::now() + TIMEOUT;
        let guarded =
            format!("(() => {{ try {{ return {expression}; }} catch {{ return null; }} }})()");
        loop {
            let value = self.eval(&guarded).await;
            if !matches!(value, Value::Null | Value::Bool(false))
                && value != json!(0)
                && value != json!("")
            {
                return value;
            }
            if Instant::now() >= deadline {
                let snapshot = self.eval(SNAPSHOT).await;
                let errors = self.errors();
                panic!("timed out waiting for {what}; page: {snapshot:#}; errors: {errors:#?}");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    async fn send_message(&mut self, text: &str) {
        let text = serde_json::to_string(text).unwrap();
        self.eval(&format!(
            "byId('input').value = {text}; byId('send').click(); true"
        ))
        .await;
    }

    /// Saves a PNG of the page as `<name>.png` in `RAH_TEST_SCREENSHOT_DIR`,
    /// when it is set, for looking at the UI; otherwise does nothing.
    async fn screenshot(&mut self, name: &str) {
        let Some(dir) = std::env::var_os("RAH_TEST_SCREENSHOT_DIR") else {
            return;
        };
        let shot = self
            .call("Page.captureScreenshot", json!({ "format": "png" }))
            .await;
        let data = shot["data"].as_str().unwrap_or_default();
        let bytes = base64_decode(data);
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.png")), bytes).unwrap();
    }

    /// Page errors: uncaught exceptions, `console.error` and browser log
    /// errors such as CSP violations and failed loads.
    fn errors(&self) -> Vec<&Value> {
        self.events
            .iter()
            .filter(|event| match event["method"].as_str() {
                Some("Runtime.exceptionThrown") => true,
                Some("Runtime.consoleAPICalled") => event["params"]["type"] == "error",
                Some("Log.entryAdded") => event["params"]["entry"]["level"] == "error",
                _ => false,
            })
            .collect()
    }
}

/// Page-side helpers, kept small: the test reads the DOM the way a user sees
/// it (visible elements and their text).
const HELPERS: &str = r#"
window.byId = (id) => document.getElementById(id);
window.shown = (node) => !!node && node.getClientRects().length > 0;
window.connected = () => !!byId('connection') && byId('connection').textContent === 'connected';
window.entries = () => [...document.querySelectorAll('#transcript .entry')];
window.transcript = () => entries().map((entry) => entry.textContent).join('\n');
window.rowOf = (id) => [...document.querySelectorAll('#plugins tr')]
  .find((row) => row.cells[0].firstChild.textContent === id);
window.rowAction = (id, label) => {
  rowOf(id).querySelector('button').click();
  [...byId('plugin-summary').querySelectorAll('button')]
    .find((button) => button.textContent === label).click();
  if (label === 'Configure' || label === 'Replace') {
    byId('editor-format').value = 'json';
    byId('editor-format').dispatchEvent(new Event('change'));
  }
};
window.approval = () => document.querySelector('#approvals .approval');
window.answer = (label) => [...approval().querySelectorAll('button')]
  .find((button) => button.textContent === label).click();
window.idle = () => !byId('send').disabled && !shown(byId('cancel')) && !shown(byId('live')) && !shown(byId('busy'));
window.sessionItems = () => [...document.querySelectorAll('#sessions li.session')];
window.folderItems = () => [...document.querySelectorAll('#sessions li.folder')];
window.treeButton = (path) => document.querySelector(`#tree button[data-path="${path}"]`);
window.changeButton = (path) => document.querySelector(`#git-body .changes button[data-path="${path}"]`);
window.treeNames = () => [...document.querySelectorAll('#tree button')].map((button) => button.dataset.path);
window.headTitle = () => byId('session-title').textContent;
window.metaAction = (label) => [...document.querySelectorAll('#transcript .meta button')]
  .filter((button) => button.textContent === label);
// Every notice the page shows, even one that is replaced right away.
window.notices = [];
// The last requests the page made, with their status or how long they have
// been pending, for the timeout report.
window.requests = [];
const pageFetch = window.fetch.bind(window);
window.fetch = (input, init) => {
  const request = { what: `${init?.method || 'GET'} ${String(input).split('?')[0]}`, start: Date.now() };
  requests.push(request);
  if (requests.length > 30) requests.shift();
  return pageFetch(input, init).then(
    (response) => { request.done = `${response.status} after ${Date.now() - request.start} ms`; return response; },
    (error) => { request.done = `${error} after ${Date.now() - request.start} ms`; throw error; },
  );
};
document.addEventListener('DOMContentLoaded', () => {
  new MutationObserver(() => {
    const text = byId('notice-text').textContent;
    if (text) notices.push(text);
  }).observe(byId('notice'), { childList: true, characterData: true, subtree: true });
});
"#;

/// What a timed-out wait reports about the page.
const SNAPSHOT: &str = r#"(async () => ({
  probe: await Promise.race([
    Promise.all([api('/plugins'), api('/info')]).then(([p, i]) => `plugins ${p.length}, tools ${i.tools.length}`, (e) => `error ${e.message}`),
    new Promise((resolve) => setTimeout(() => resolve('hung'), 3000)),
  ]),
  notices: window.notices,
  notice: document.getElementById('notice-text')?.textContent,
  connection: document.getElementById('connection')?.textContent,
  plugins: [...document.querySelectorAll('#plugins tr')].map((row) => [...row.cells].map((cell) => cell.textContent)),
  approvals: document.getElementById('approvals')?.textContent,
  live: document.getElementById('live')?.textContent,
  transcript: [...document.querySelectorAll('#transcript .entry')].map((entry) => entry.textContent),
  session: { current: state.current, running: state.running, starting: !!state.starting, workspace: state.workspace, title: document.getElementById('session-title')?.textContent },
  sessions: [...document.querySelectorAll('#sessions li.session')].map((item) => item.dataset.id + ' ' + item.textContent),
  requests: window.requests.map((r) => `${r.what}: ${r.done || `pending for ${Date.now() - r.start} ms`}`),
}))()"#;

struct Web {
    _kernel: Kernel,
    url: String,
    dir: TempDir,
}

async fn start(name: &str) -> Web {
    start_with(name, |_, _| {}).await
}

/// Like [`start`]; `adjust` changes the distribution first (it gets the
/// test's directory too).
async fn start_with(name: &str, adjust: impl FnOnce(&mut Distribution, &TempDir)) -> Web {
    let dir = TempDir::new(name);
    // A configuration file, so settings saved in the page are kept (ADR 0067).
    let mut distribution = from_file(&dir);
    distribution.plugin.push(
        Distribution::parse(
            r#"
[[plugin]]
use = "frontend-web"
config = { bind = "127.0.0.1:0" }
"#,
        )
        .unwrap()
        .plugin
        .remove(0),
    );
    adjust(&mut distribution, &dir);
    let kernel = Kernel::new();
    let url = Arc::new(Mutex::new(None));
    let seen = url.clone();
    let _subscription = kernel
        .handle()
        .subscribe(move |listening: &FrontendListening| {
            *seen.lock().unwrap() = Some(listening.url.clone());
        });
    boot(&kernel, &distribution).await.unwrap();
    let url = url
        .lock()
        .unwrap()
        .clone()
        .expect("frontend-web announced its URL");
    Web {
        _kernel: kernel,
        url,
        dir,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn plugins_share_a_settings_workspace_and_preserve_configuration_values() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start_with("web-plugin-settings", |distribution, _| {
        distribution.set_config("model-mock", "label", json!("Before"));
    })
    .await;
    let browser = Browser::launch(PathBuf::from(program), "web-plugin-settings");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("openSettings('plugins'); true").await;
    page.wait(
        "the shared workspace",
        "byId('settings-body').contains(byId('plugin-workspace')) && !!rowOf('model-mock')",
    )
    .await;
    page.eval("rowOf('model-mock').querySelector('button').click(); true")
        .await;
    assert_eq!(page.eval("byId('editor-format').value").await, "fields");
    page.screenshot("unified-plugin-fields").await;
    page.eval(
        "window.format = (value) => { byId('editor-format').value = value; byId('editor-format').dispatchEvent(new Event('change')); }; \
         window.configField = (key) => [...document.querySelectorAll('#editor-fields [data-config-key]')].find((input) => input.dataset.configKey === key); \
         window.editField = (key, value) => { const input = configField(key); input.value = value; input.dispatchEvent(new Event('input', { bubbles: true })); }; true",
    ).await;

    // Round trips preserve unknown keys, nested data, false, zero, null, empty
    // strings and prototype-like keys. Only the edited setting changes.
    page.eval(
        "format('json'); byId('editor-config').value = '{\"label\":\"Before\",\"unknown\":{\"nested\":[1,false,null]},\"off\":false,\"zero\":0,\"empty\":\"\",\"nil\":null,\"__proto__\":{\"safe\":true}}'; \
         window.originalConfig = JSON.parse(byId('editor-config').value); format('fields'); editField('label', 'After'); format('json'); true",
    ).await;
    assert_eq!(
        page.eval("JSON.stringify(JSON.parse(byId('editor-config').value)) === JSON.stringify({ ...originalConfig, label: 'After' })").await,
        true
    );
    // Unset values stay absent; the example never becomes a value by rendering.
    page.eval("byId('editor-config').value = '{}'; format('fields'); true")
        .await;
    assert_eq!(page.eval("configField('label').disabled").await, true);
    assert_eq!(page.eval("byId('editor-config').value").await, "{}");
    page.eval(
        "document.querySelector('#editor-fields input[aria-label=\"Set label\"]').click(); editField('label', 'After'); byId('editor-save').click(); true",
    ).await;
    page.wait(
        "the field save",
        "byId('plugin-message').textContent.includes('configuration saved') && byId('editor').hidden",
    ).await;
    assert_eq!(
        page.eval("api('/plugins').then((plugins) => plugins.find((plugin) => plugin.id === 'model-mock').config.label)").await,
        "After"
    );
    page.eval("rowOf('model-mock').querySelector('button').click(); format('json'); byId('editor-config').value = '{ invalid'; format('fields'); true").await;
    assert_eq!(page.eval("byId('editor-format').value").await, "json");
    assert_eq!(
        page.eval("byId('plugin-message').classList.contains('bad')")
            .await,
        true
    );
    page.eval("byId('editor-config').value = 'false'; format('fields'); true")
        .await;
    assert_eq!(page.eval("byId('editor-format').value").await, "json");
    assert_eq!(page.eval("byId('editor-config').value").await, "false");
    // Server validation is shown locally and keeps both the old config and draft.
    page.eval("byId('editor-config').value = '{\"label\":123}'; byId('editor-save').click(); true")
        .await;
    page.wait("the refused configuration", "byId('plugin-message').classList.contains('bad') && byId('plugin-message').textContent.includes('string')").await;
    assert_eq!(
        page.eval("shown(byId('editor')) && byId('editor-config').value === '{\"label\":123}'")
            .await,
        true
    );
    assert_eq!(page.eval("api('/plugins').then((plugins) => plugins.find((plugin) => plugin.id === 'model-mock').config.label)").await, "After");

    page.eval("byId('plugin-search').value = 'model-mock'; byId('plugin-search').dispatchEvent(new Event('input')); true").await;
    assert_eq!(
        page.eval("rowOf('policy-authority').hidden && !rowOf('model-mock').hidden")
            .await,
        true
    );
    page.screenshot("unified-plugin-settings").await;
    page.call(
        "Emulation.setDeviceMetricsOverride",
        json!({ "width": 390, "height": 844, "deviceScaleFactor": 1, "mobile": false }),
    )
    .await;
    assert_eq!(
        page.eval("document.documentElement.scrollWidth <= innerWidth")
            .await,
        true
    );
    page.screenshot("unified-plugin-settings-mobile").await;
    // Escape/close puts the one workspace back in its layout panel.
    page.eval("byId('settings-dialog').close(); true").await;
    page.wait(
        "the restored workspace",
        "byId('plugins-panel').contains(byId('plugin-workspace'))",
    )
    .await;
    // The fallback remains usable when a layout omits the Plugins panel.
    page.eval("layout.right = layout.right.filter((id) => id !== 'plugins'); layout.left = layout.left.filter((id) => id !== 'plugins'); showPlugins(); true").await;
    page.wait(
        "settings without a Plugins panel",
        "byId('settings-dialog').open && byId('settings-body').contains(byId('plugin-workspace'))",
    )
    .await;
    let exceptions: Vec<_> = page
        .errors()
        .into_iter()
        .filter(|event| event["method"] == "Runtime.exceptionThrown")
        .collect();
    assert!(exceptions.is_empty(), "page exceptions: {exceptions:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn example_personal_plugins_import_enable_configure_and_run_in_the_workspace() {
    use rah_api::tools::{Invocation, ToolInvoker};
    use rah_protocol::{ToolCall, ToolCallId, ToolInput, ToolName};

    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-personal-examples").await;
    let examples =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/personal-plugins");
    let bundle = web.dir.0.join("examples.json");
    let built = Command::new("node")
        .arg(examples.join("bundle.mjs"))
        .arg(&bundle)
        .output()
        .expect("Node.js builds the example bundle");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let browser = Browser::launch(PathBuf::from(program), "web-personal-examples");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("openSettings('plugins'); true").await;
    page.wait("My plugins in the shared workspace", "[...byId('plugin-pages').querySelectorAll('button')].some((button) => button.textContent === 'My plugins')").await;
    page.eval(
        "window.pickPluginPage = (title) => [...byId('plugin-pages').querySelectorAll('button')].find((button) => button.textContent === title).click(); \
         window.pluginPageButton = (label) => [...byId('plugin-page').querySelectorAll('button')].find((button) => button.textContent === label); \
         pickPluginPage('My plugins'); true",
    ).await;
    page.wait(
        "the compact personal library",
        "!!pluginPageButton('Import')",
    )
    .await;
    assert_eq!(
        page.eval("byId('plugin-page').querySelectorAll('form').length")
            .await,
        0
    );
    page.eval("pluginPageButton('Import').click(); true").await;
    page.wait(
        "the import form",
        "!!byId('plugin-page').querySelector('input[name=file]')",
    )
    .await;
    let file = serde_json::to_string(&bundle.display().to_string()).unwrap();
    page.eval(&format!("byId('plugin-page').querySelector('input[name=file]').value = {file}; pluginPageButton('Import').click(); true")).await;
    page.wait("the imported plugins", "!!pluginPageButton('text-lab') && !!pluginPageButton('scratch-note') && !!pluginPageButton('tally-board')").await;
    assert_eq!(
        page.eval("byId('plugin-page').querySelectorAll('.view-row-select').length === 3 && byId('plugin-page').querySelectorAll('form').length === 0 && !byId('plugin-page').textContent.includes('Manage:')").await,
        true
    );
    page.screenshot("compact-personal-library").await;
    page.eval("pluginPageButton('Backups & export').click(); true")
        .await;
    page.wait(
        "the separate transfer page",
        "!!pluginPageButton('Export') && !!pluginPageButton('Back to the list')",
    )
    .await;
    page.eval("pluginPageButton('Back to the list').click(); true")
        .await;
    page.wait(
        "the compact library again",
        "!!pluginPageButton('text-lab')",
    )
    .await;
    page.eval("byId('settings-dialog').close(); showPanel('plugin-builder/plugins'); true")
        .await;
    page.wait("the compact sidebar library", "views.get('plugin-builder/plugins').body.querySelectorAll('.view-row-select').length === 3 && views.get('plugin-builder/plugins').body.querySelectorAll('form').length === 0").await;
    page.screenshot("compact-my-plugins-sidebar").await;
    page.eval("openSettings('plugins'); true").await;
    page.wait(
        "the reopened workspace",
        "byId('settings-body').contains(byId('plugin-workspace'))",
    )
    .await;
    assert_eq!(
        page.eval("!rowOf('text-lab') && !rowOf('scratch-note') && !rowOf('tally-board')")
            .await,
        true,
        "Import never mounts a plugin"
    );
    for name in ["text-lab", "scratch-note", "tally-board"] {
        page.eval(&format!("pluginPageButton('{name}').click(); true"))
            .await;
        page.wait(
            "the enable confirmation",
            "!!byId('plugin-page').querySelector('input[name=read]')",
        )
        .await;
        page.eval("byId('plugin-page').querySelector('input[name=read]').click(); pluginPageButton('Enable').click(); true").await;
        page.wait("the enabled plugin", &format!("!!rowOf('{name}') && rowOf('{name}').cells[1].textContent === 'active' && !!pluginPageButton('Back to the list')")).await;
        page.eval("pluginPageButton('Back to the list').click(); true")
            .await;
        page.wait("the saved plugin list", "!!pluginPageButton('text-lab')")
            .await;
    }
    // Personal plugins use the same field editor, including nested config.
    page.eval("rowOf('text-lab').querySelector('button').click(); true")
        .await;
    page.eval(
        "document.querySelector('#editor-fields input[aria-label=\"Set plugin\"]').click(); \
         { const input = document.querySelector('#editor-fields [data-config-key=plugin]'); input.value = '{\"label\":\"UI test\"}'; input.dispatchEvent(new Event('input')); } \
         byId('editor-save').click(); true",
    ).await;
    page.wait(
        "the personal plugin config save",
        "byId('plugin-message').textContent.includes('configuration saved')",
    )
    .await;
    let handle = web._kernel.handle();
    let session = common::new_session(&handle).await;
    let invocation = |name: &str, input: Value| Invocation {
        session: session.clone(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(input),
        },
        cancel: rah_api::CancellationToken::new(),
    };
    let invoker = ToolInvoker::new(handle.clone());
    let result = invoker
        .invoke(
            invocation("text-lab.inspect", json!({ "text": "hello world" })),
            || {},
        )
        .await;
    assert!(!result.output.is_error, "{result:?}");
    let result = serde_json::to_value(&result.output).unwrap();
    assert!(result.to_string().contains("UI test"), "{result}");
    // Writes remain behind policy: grant this exact test tool read/write
    // levels through the existing admin configuration, never by metadata.
    let admin = handle.get(rah_api::admin::ADMIN).unwrap();
    let policy_id = rah_api::PluginId::new("policy-authority");
    let mut policy = admin.instance(&policy_id).unwrap().config;
    policy["decisions"]["scratch-note.write"] = json!("allow");
    admin.configure(&policy_id, policy).await.unwrap();
    let written = invoker
        .invoke(
            invocation("scratch-note.write", json!({ "text": "A saved test note" })),
            || {},
        )
        .await;
    assert!(!written.output.is_error, "{written:?}");
    admin
        .restart(&rah_api::PluginId::new("scratch-note"))
        .await
        .unwrap();
    let read = invoker
        .invoke(invocation("scratch-note.read", json!({})), || {})
        .await;
    assert!(!read.output.is_error, "{read:?}");
    assert!(
        serde_json::to_value(read.output)
            .unwrap()
            .to_string()
            .contains("A saved test note")
    );
    page.eval("pickPluginPage('Tally board'); true").await;
    page.wait(
        "the personal board",
        "byId('plugin-page').textContent.includes('Count: 0')",
    )
    .await;
    page.eval("pluginPageButton('Add one').click(); true").await;
    page.wait(
        "the board action",
        "byId('plugin-page').textContent.includes('Count: 1')",
    )
    .await;
    assert_eq!(
        page.eval("(() => { const count = [...byId('plugin-page').querySelectorAll('p')].find((node) => node.textContent === 'Count: 1').getBoundingClientRect(); const body = byId('settings-body').getBoundingClientRect(); return count.top >= body.top && count.bottom <= body.bottom; })()")
            .await,
        true,
        "A long plugin list must not scroll the selected page out of sight"
    );
    page.screenshot("personal-test-plugins").await;
    page.eval("pickPluginPage('My plugins'); true").await;
    page.wait("the management list", "!!pluginPageButton('tally-board')")
        .await;
    page.eval("pluginPageButton('tally-board').click(); true")
        .await;
    page.wait("the disable action", "!!pluginPageButton('Disable')")
        .await;
    page.eval("pluginPageButton('Disable').click(); true").await;
    page.wait(
        "the disabled personal plugin",
        "!rowOf('tally-board') && !!pluginPageButton('Enable')",
    )
    .await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_web_ui_works_in_a_real_browser() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser").await;
    let browser = Browser::launch(PathBuf::from(program), "web-browser");
    let mut page = browser.page().await;

    // Opening the printed URL: the token leaves the address bar and the
    // socket connects.
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    assert_eq!(page.eval("location.hash").await, "");

    // The status line: workspace, Git state (not a repository here) and
    // the providers the next turn uses.
    page.wait(
        "the status line",
        "shown(byId('status')) && byId('status-model').textContent === 'model: model-mock'",
    )
    .await;
    let status = page
        .eval(
            "['status-root', 'status-git', 'status-agent'] \
             .map((id) => byId(id).textContent)",
        )
        .await;
    assert!(
        status[0].as_str().unwrap().ends_with("workspace"),
        "{status}"
    );
    assert_eq!(status[1], "no git repository", "{status}");
    // The usual loop and a policy are not named; the title tells them.
    let agent = status[2].as_str().unwrap();
    assert!(
        agent.ends_with(" tools") && !agent.contains('·'),
        "{status}"
    );
    let title = page.eval("byId('status-agent').title").await;
    let title = title.as_str().unwrap();
    assert!(
        title.contains("loop plugin loop-react") && title.contains("Guarded"),
        "{title}"
    );
    assert_eq!(page.eval("byId('input').disabled").await, true);
    assert_eq!(page.eval("shown(byId('notice'))").await, false);

    // A session and a turn with a tool call.
    page.eval("byId('new-session').click(); true").await;
    page.wait(
        "the new session",
        "document.querySelectorAll('#sessions li.session').length === 1 && !byId('input').disabled",
    )
    .await;
    page.send_message("hello").await;
    page.wait(
        "the first reply",
        "transcript().includes('[model-mock] echo returned: hello') && idle()",
    )
    .await;
    let kinds = page
        .eval("entries().map((entry) => entry.querySelector('.meta span').textContent)")
        .await;
    assert_eq!(
        kinds,
        json!(["You", "tool call · echo", "tool result", "Assistant"])
    );
    assert_eq!(page.eval("byId('input').value").await, "");
    // model-mock reports usage for both model requests of the turn.
    assert_eq!(page.eval("shown(byId('status-usage'))").await, true);
    let usage = page.eval("byId('status-usage').textContent").await;
    assert!(
        usage
            .as_str()
            .is_some_and(|text| text.ends_with(" out") && text.contains(" in · ")),
        "{usage}"
    );
    let title = page.eval("byId('status-usage').title").await;
    assert!(
        title
            .as_str()
            .is_some_and(|text| text.ends_with("2 model requests")),
        "{title}"
    );

    // Enter sends; the textarea does not keep a newline.
    page.eval(
        "byId('input').value = '/notool second'; \
         byId('input').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })); true",
    )
    .await;
    page.wait(
        "the Enter reply",
        "transcript().includes('[model-mock] you said: second') && idle()",
    )
    .await;
    assert_eq!(page.eval("entries().length").await, 6);
    assert_eq!(page.eval("byId('input').value").await, "");

    // Fork after the first record: a second, current session with only it.
    let original = page
        .eval("document.querySelector('#sessions li.current').title")
        .await;
    page.eval("metaAction('fork here')[0].click(); true").await;
    page.wait(
        "the fork",
        "document.querySelectorAll('#sessions li.session').length === 2 && entries().length === 1",
    )
    .await;
    let current = page
        .eval("document.querySelector('#sessions li.current').title")
        .await;
    assert_ne!(current, original);
    assert!(
        page.eval("document.querySelector('#sessions li.current small').textContent")
            .await
            .as_str()
            .unwrap()
            .contains("fork of ")
    );

    // Back to the original session.
    let original_text = serde_json::to_string(&original).unwrap();
    page.eval(&format!(
        "[...document.querySelectorAll('#sessions li.session')].find((item) => item.title === {original_text}).click(); true"
    ))
    .await;
    page.wait("the original transcript", "entries().length === 6")
        .await;

    // The Plugins panel.
    assert_eq!(page.eval("shown(byId('plugins-panel'))").await, false);
    page.eval("byId('status-agent').click(); true").await;
    page.wait(
        "the plugin table",
        "shown(byId('plugins-panel')) && !!rowOf('policy-authority')",
    )
    .await;
    assert_eq!(
        page.eval("rowOf('policy-authority').cells[1].textContent")
            .await,
        "active",
        "{}",
        page.eval("byId('plugins').innerHTML").await
    );
    page.eval("rowOf('distribution').querySelector('button').click(); true")
        .await;
    assert_eq!(
        page.eval("byId('editor').hidden && byId('plugin-summary').querySelectorAll('button').length === 0")
            .await,
        true
    );
    assert_eq!(
        page.eval(
            "[...byId('tools').children].some((item) => item.textContent.startsWith('echo ('))"
        )
        .await,
        true
    );

    // Configure the policy so `echo` asks, through the editor.
    page.eval("rowAction('policy-authority', 'Configure'); true")
        .await;
    page.wait("the editor", "shown(byId('editor'))").await;
    assert_eq!(page.eval("shown(byId('editor-kind-row'))").await, false);
    page.eval(
        "{ const config = JSON.parse(byId('editor-config').value); \
         config.tools = { ...config.tools, echo: 'write' }; \
         byId('editor-config').value = JSON.stringify(config); \
         byId('editor-save').click(); true }",
    )
    .await;
    page.wait(
        "the policy to restart",
        "!shown(byId('editor')) && !shown(byId('notice')) && \
         rowOf('policy-authority').cells[1].textContent === 'active'",
    )
    .await;

    // Allow, deny and cancel an approval.
    page.send_message("allowed call").await;
    page.wait(
        "the approval card",
        "shown(approval()) && approval().textContent.includes('Allow echo?')",
    )
    .await;
    assert_eq!(page.eval("shown(byId('cancel'))").await, true);
    // The busy line says what the turn waits for, with a running clock, and
    // the tab title is marked.
    page.wait(
        "the busy line",
        "shown(byId('busy')) && byId('busy-text').textContent === 'Waiting for your approval' && \
         byId('busy').classList.contains('waiting') && /^\\d+s$/.test(byId('busy-time').textContent)",
    )
    .await;
    page.screenshot("busy-approval").await;
    // The title names the version.
    let title = format!("RAH {}", env!("CARGO_PKG_VERSION"));
    assert_eq!(
        page.eval("document.title").await,
        format!("● {title} — working")
    );
    assert_eq!(
        page.eval("approval().querySelector('div').textContent")
            .await,
        "tool `echo` requires `write` permission"
    );
    page.eval("answer('Allow'); true").await;
    page.wait(
        "the allowed reply",
        "transcript().includes('echo returned: allowed call') && idle() && !approval()",
    )
    .await;
    assert_eq!(page.eval("document.title").await, title.as_str());

    page.send_message("denied call").await;
    page.wait("the second approval card", "shown(approval())")
        .await;
    page.eval("answer('Deny'); true").await;
    page.wait(
        "the denied reply",
        "transcript().includes('denied: rejected by the user') && idle() && !approval()",
    )
    .await;

    page.send_message("cancelled call").await;
    page.wait("the third approval card", "shown(approval())")
        .await;
    page.eval("byId('cancel').click(); true").await;
    page.wait(
        "the cancelled turn",
        "idle() && !approval() && transcript().includes('cancelled before the tool started')",
    )
    .await;
    assert_eq!(page.eval("shown(byId('notice'))").await, false);

    // Replace the model through the editor; the next turn uses it.
    page.eval("rowAction('model-mock', 'Replace'); true").await;
    page.wait("the replace editor", "shown(byId('editor-kind-row'))")
        .await;
    assert_eq!(page.eval("byId('editor-kind').value").await, "model-mock");
    page.eval(
        "byId('editor-config').value = JSON.stringify({ label: 'B' }); \
         byId('editor-save').click(); true",
    )
    .await;
    page.wait(
        "the replacement model",
        "!shown(byId('editor')) && !!rowOf('model-mock-2') && !rowOf('model-mock')",
    )
    .await;
    page.wait(
        "the status line to follow the replacement",
        "byId('status-model').textContent === 'model: model-mock-2'",
    )
    .await;
    page.wait(
        "the reconnect notice",
        "byId('notice-text').textContent === 'Reconnected to model model-mock-2. The next message uses it.'",
    )
    .await;
    page.send_message("/notool after replace").await;
    page.wait(
        "the replacement reply",
        "transcript().includes('[B] you said: after replace') && idle()",
    )
    .await;
    assert_eq!(page.eval("shown(byId('notice'))").await, false);

    // Streaming fragments remain literal text and keep the existing text node.
    let fragments = page
        .eval(
            "(() => { setRunning(false); showLive('first'); \
             const live = byId('live'); const original = live.firstChild; \
             for (let i = 0; i < 1000; i++) showLive('中😀<b>x</b>'); \
             const result = { exact: live.textContent === 'first' + '中😀<b>x</b>'.repeat(1000), \
               reused: live.firstChild === original && live.childNodes.length === 1, \
               literal: live.querySelector('b') === null }; \
             setRunning(false); result.cleared = live.childNodes.length === 0 && live.hidden; \
             showLive('next'); result.nextTurn = live.textContent === 'next'; \
             setRunning(false); return result; })()",
        )
        .await;
    assert_eq!(
        fragments,
        json!({ "exact": true, "reused": true, "literal": true, "cleared": true, "nextTurn": true })
    );

    // Compare DOM mutation cost only; timing is diagnostic, never a test gate.
    let mutation_cost = page
        .eval(
            "(() => { const fragment = 'x'.repeat(64); const count = 3000; \
             const measure = (append) => { const box = document.createElement('pre'); \
               box.append(document.createTextNode('')); let text = ''; \
               const start = performance.now(); \
               for (let i = 0; i < count; i++) { \
                 if (append) box.firstChild.appendData(fragment); \
                 else { text += fragment; box.textContent = text; } \
               } \
               const ms = performance.now() - start; \
               if (box.textContent !== fragment.repeat(count)) throw Error('lost fragment'); \
               return ms; }; \
             measure(false); measure(true); \
             const replacement = []; const append = []; \
             for (let i = 0; i < 3; i++) { \
               if (i % 2) { append.push(measure(true)); replacement.push(measure(false)); } \
               else { replacement.push(measure(false)); append.push(measure(true)); } \
             } \
             return { fragments: count, fragmentBytes: fragment.length, replacementMs: replacement, appendMs: append }; })()",
        )
        .await;
    eprintln!("live output DOM mutation comparison: {mutation_cost}");

    // Long streaming output scrolls inside the conversation: the page does not
    // grow and the status line stays at the bottom of the window.
    let layout = page
        .eval(
            "(() => { showLive('line\\n'.repeat(500)); \
             const live = byId('live'); \
             const result = { \
               pageFits: document.documentElement.scrollHeight <= innerHeight, \
               pageFitsWidth: document.documentElement.scrollWidth <= innerWidth, \
               statusAtBottom: Math.abs(byId('status').getBoundingClientRect().bottom - innerHeight) < 2, \
               followsOutput: live.scrollHeight - live.scrollTop - live.clientHeight < 2, \
             }; \
             live.scrollTop = 0; showLive('more\\n'); \
             result.preservesScroll = live.scrollTop === 0; \
             setRunning(false); return result; })()",
        )
        .await;
    assert_eq!(
        layout,
        json!({ "pageFits": true, "pageFitsWidth": true, "statusAtBottom": true, "followsOutput": true, "preservesScroll": true })
    );

    // Add a plugin, then remove it after the confirmation.
    page.eval("byId('add-plugin').click(); true").await;
    page.wait("the add editor", "shown(byId('editor-id-row'))")
        .await;
    // The empty configuration shows the chosen kind's example as a hint.
    assert_eq!(
        page.eval(
            "const kind = byId('editor-kind'); kind.value = 'loop-react'; \
             kind.dispatchEvent(new Event('change')); \
             byId('editor-config').value === '' && \
             byId('editor-config').placeholder.includes('\"max_rounds\": 32')"
        )
        .await,
        true
    );
    // "Fill in example" copies it; an example value still in angle brackets
    // is refused at save.
    assert_eq!(
        page.eval(
            "byId('editor-example').click(); \
             JSON.parse(byId('editor-config').value).max_rounds === 32"
        )
        .await,
        true
    );
    page.eval(
        "window.confirm = () => true; byId('editor-kind').value = 'context-compact'; \
         byId('editor-kind').dispatchEvent(new Event('change')); \
         byId('editor-example').click(); byId('editor-save').click(); true",
    )
    .await;
    page.wait(
        "the refused example value",
        "shown(byId('plugin-message')) && byId('plugin-message').textContent.includes('<provider model id>') \
         && shown(byId('editor')) && !rowOf('context-compact')",
    )
    .await;
    page.eval(
        "byId('editor-kind').value = 'model-mock'; byId('editor-id').value = 'spare'; \
         byId('editor-config').value = '{}'; byId('editor-save').click(); true",
    )
    .await;
    page.wait("the added plugin", "!!rowOf('spare')").await;
    page.eval("window.confirm = () => true; rowAction('spare', 'Remove'); true")
        .await;
    page.wait("the removed plugin", "!rowOf('spare')").await;

    // Invalid JSON in the editor is reported and changes nothing.
    page.eval("rowAction('model-mock-2', 'Configure'); true")
        .await;
    page.eval("byId('editor-config').value = '{ nope'; byId('editor-save').click(); true")
        .await;
    page.wait(
        "the JSON error",
        "shown(byId('plugin-message')) && byId('plugin-message').textContent.includes('not valid JSON')",
    )
    .await;
    page.eval("byId('editor-cancel').click(); true").await;
    let notices = page.eval("notices").await;
    // Editor validation stays beside the editor; only reconnect is global.
    assert_eq!(notices.as_array().unwrap().len(), 1, "notices: {notices:#}");

    // A reload keeps the token for the tab, without the fragment.
    page.call("Page.reload", json!({})).await;
    page.wait(
        "the page to reconnect",
        "connected() && entries().length > 0",
    )
    .await;
    assert_eq!(
        page.eval("document.querySelectorAll('#sessions li.session').length")
            .await,
        2
    );
    assert_eq!(page.eval("notices").await, json!([]));

    // Flush pending events, then require a clean console.
    page.eval("true").await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");

    // Slash commands: `/` lists them, Tab completes, plugin ids complete
    // after `/stop `, and Enter runs the command in the browser.
    let typing = |text: &str| {
        let text = serde_json::to_string(text).unwrap();
        format!(
            "byId('input').value = {text}; byId('input').dispatchEvent(new Event('input')); true"
        )
    };
    let key = |key: &str| {
        format!(
            "byId('input').dispatchEvent(new KeyboardEvent('keydown', {{ key: '{key}', bubbles: true, cancelable: true }})); true"
        )
    };
    let menu = "[...document.querySelectorAll('#slash-menu .slash-item code')].map((item) => item.textContent)";
    page.eval(&typing("/")).await;
    page.wait("the command menu", "shown(byId('slash-menu'))")
        .await;
    assert_eq!(
        page.eval(&format!("{menu}.length")).await,
        10,
        "{}",
        page.eval(menu).await
    );
    page.screenshot("slash-menu").await;
    page.eval(&key("Escape")).await;
    assert_eq!(page.eval("shown(byId('slash-menu'))").await, false);
    page.eval(&typing("/st")).await;
    assert_eq!(
        page.eval(menu).await,
        json!(["/stop <plugin>", "/start <plugin>"])
    );
    page.eval(&key("Tab")).await;
    assert_eq!(page.eval("byId('input').value").await, "/stop ");
    page.wait(
        "plugin ids to complete",
        &format!("{menu}.includes('model-mock-2')"),
    )
    .await;
    assert_eq!(
        page.eval(&format!("{menu}.includes('policy-authority')"))
            .await,
        false
    );
    page.eval(&typing("/stop model-mock-2")).await;
    page.eval(&key("Enter")).await;
    page.wait(
        "the stopped plugin",
        "rowOf('model-mock-2').cells[1].textContent === 'stopped' && \
         notices.includes('model-mock-2: stopped') && byId('input').value === ''",
    )
    .await;
    page.eval("rowAction('model-mock-2', 'Start'); true").await;
    page.wait(
        "the started plugin",
        "rowOf('model-mock-2').cells[1].textContent === 'active'",
    )
    .await;
    page.eval(&typing("/bogus")).await;
    assert_eq!(page.eval("shown(byId('slash-menu'))").await, false);
    page.eval(&key("Enter")).await;
    // Shown at least once: a status refresh (such as the model switch
    // above) may replace it right after.
    page.wait(
        "the unknown command notice",
        "notices.includes('Unknown command /bogus. Type / to see the commands.')",
    )
    .await;
    page.eval("true").await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");

    // A tool result shows its raster images; other image types stay text.
    let shown = page
        .eval(
            "(() => { const node = entryNode({ seq: 99, entry: { kind: 'tool_result', effect: 'none', \
             output: { is_error: false, content: [{ type: 'text', value: 'window' }, \
             { type: 'image', value: { media_type: 'image/png', data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=' } }, \
             { type: 'image', value: { media_type: 'image/svg+xml', data: 'PHN2Zz4=' } }] } } }); \
             return { text: node.querySelector('pre').textContent, \
             images: [...node.querySelectorAll('img')].map((image) => image.getAttribute('src')) }; })()",
        )
        .await;
    assert_eq!(
        shown,
        json!({
            "text": "window\n[image/png image]\n[image/svg+xml image]",
            "images": ["data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII="]
        })
    );

    // A failed startup request stays reported after the socket connects.
    page.eval("window.confirm = () => true; rowAction('session-jsonl', 'Remove'); true")
        .await;
    page.wait("the removed session plugin", "!rowOf('session-jsonl')")
        .await;
    page.call("Page.reload", json!({})).await;
    page.wait(
        "the startup error after connecting",
        "connected() && shown(byId('notice')) && \
         byId('notice-text').textContent.includes('no session service is mounted')",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sessions_are_named_renamed_branched_exported_and_deleted_in_the_browser() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-sessions").await;
    let browser = Browser::launch(PathBuf::from(program), "web-browser-sessions");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    assert_eq!(page.eval("shown(byId('session-head'))").await, false);

    // The first turn gets a generated title (the mock model's reply line).
    page.eval("byId('new-session').click(); true").await;
    page.wait(
        "the new session",
        "sessionItems().length === 1 && !byId('input').disabled",
    )
    .await;
    page.send_message("hello").await;
    page.wait(
        "the generated title",
        "idle() && headTitle().includes('you said: User: hello') && \
         sessionItems()[0].querySelector('.name').textContent === headTitle()",
    )
    .await;

    // Rename from the header; the next turn keeps the title.
    page.eval("byId('session-rename').click(); true").await;
    page.eval(
        "{ const input = document.querySelector('#session-title .title-input'); \
         input.value = 'Kyoto trip'; \
         input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); true }",
    )
    .await;
    page.wait("the new title", "headTitle() === 'Kyoto trip'")
        .await;
    page.send_message("/notool second").await;
    page.wait(
        "the second reply",
        "idle() && transcript().includes('you said: second')",
    )
    .await;
    assert_eq!(page.eval("headTitle()").await, "Kyoto trip");
    let original = page.eval("state.current").await;

    // Find a later message even though the title and first-message preview differ.
    page.eval("byId('session-search').value = 'SECOND'; byId('session-search').dispatchEvent(new Event('input')); true").await;
    page.wait("a full-history result", "!historySearchPending && document.querySelector('.search-snippet')?.textContent.includes('second')").await;
    page.eval("sessionItems()[0].click(); true").await;
    page.wait(
        "the matching message opens",
        "!byId('input').disabled && entries().length > 2",
    )
    .await;
    page.eval("byId('session-search').value = ''; byId('session-search').dispatchEvent(new Event('input')); true").await;

    // Regenerate the last answer: a new branch with the same title.
    assert_eq!(page.eval("metaAction('regenerate').length").await, 1);
    page.eval("metaAction('regenerate')[0].click(); true").await;
    page.wait(
        "the regenerated answer",
        "idle() && sessionItems().length === 2 && entries().length === 6 && \
         transcript().includes('you said: second') && headTitle() === 'Kyoto trip'",
    )
    .await;
    assert_ne!(page.eval("state.current").await, original);

    // Edit the first message and send it again in a new branch.
    page.eval("metaAction('edit')[0].click(); true").await;
    page.eval(
        "{ const editor = document.querySelector('#transcript .edit'); \
         editor.querySelector('textarea').value = '/notool edited'; \
         [...editor.querySelectorAll('button')].find((b) => b.textContent === 'Send').click(); true }",
    )
    .await;
    page.wait(
        "the edited branch",
        "idle() && sessionItems().length === 3 && entries().length === 2 && \
         transcript().includes('you said: edited')",
    )
    .await;

    // Search filters the list.
    page.eval(
        "byId('session-search').value = 'no-such-session'; \
         byId('session-search').dispatchEvent(new Event('input')); true",
    )
    .await;
    page.wait("history search completes", "!historySearchPending")
        .await;
    assert_eq!(page.eval("sessionItems().length").await, 0);
    assert_eq!(
        page.eval("byId('sessions').textContent.includes('No matching sessions')")
            .await,
        true
    );
    page.eval(
        "byId('session-search').value = 'KYOTO'; \
         byId('session-search').dispatchEvent(new Event('input')); true",
    )
    .await;
    assert_eq!(page.eval("sessionItems().length").await, 3);
    page.eval(
        "byId('session-search').value = ''; \
         byId('session-search').dispatchEvent(new Event('input')); true",
    )
    .await;

    // Export: the download is produced, and its Markdown starts with the title.
    let markdown = page
        .eval(
            "fetch(`/api/sessions/${state.current}/export?format=md`, \
             { headers: { Authorization: `Bearer ${state.token}` } }).then((r) => r.text())",
        )
        .await;
    assert!(
        markdown.as_str().unwrap().starts_with("# Kyoto trip\n"),
        "{markdown}"
    );
    // The buttons' handler, awaited so the exports finish before the delete.
    // With a save dialog, the file goes where the user picks; the stub
    // records what would be written.
    // They are in the session menu, hidden until it opens.
    assert_eq!(page.eval("shown(byId('export-md'))").await, false);
    assert_eq!(
        page.eval(
            "byId('session-menu').querySelector('summary').click(); \
             shown(byId('export-md')) && shown(byId('export-json'))"
        )
        .await,
        true
    );
    page.screenshot("session-menu").await;
    page.eval(
        "window.saved = [];          window.showSaveFilePicker = async (options) => ({ createWritable: async () => {            const parts = [];            return { write: async (part) => { parts.push(part); }, abort: async () => {},              close: async () => { saved.push({ name: options.suggestedName,                type: Object.keys(options.types[0].accept)[0],                text: await new Blob(parts).text() }); } }; } }); true",
    )
    .await;
    page.eval("Promise.all([exportSession('md'), exportSession('json')]).then(() => true)")
        .await;
    let saved = page.eval("saved").await;
    let current = page.eval("state.current").await;
    let current = current.as_str().unwrap();
    let saved = saved.as_array().unwrap();
    assert_eq!(saved.len(), 2, "{saved:#?}");
    let md = saved
        .iter()
        .find(|file| file["type"] == "text/markdown")
        .unwrap();
    assert_eq!(md["name"], format!("rah-session-{current}.md"));
    assert!(md["text"].as_str().unwrap().starts_with(
        "# Kyoto trip
"
    ));
    let json_file = saved
        .iter()
        .find(|file| file["type"] == "application/json")
        .unwrap();
    assert_eq!(json_file["name"], format!("rah-session-{current}.json"));
    serde_json::from_str::<Value>(json_file["text"].as_str().unwrap()).unwrap();

    // Cancelling the dialog saves nothing and reports nothing.
    page.eval(
        "window.showSaveFilePicker = async () => {            throw new DOMException('The user aborted a request.', 'AbortError'); };          exportSession('md').then(() => true)",
    )
    .await;
    assert_eq!(page.eval("saved.length").await, 2);

    // Without a save dialog, the browser downloads the file.
    page.eval("delete window.showSaveFilePicker; exportSession('md').then(() => true)")
        .await;
    let download = browser
        .downloads()
        .join(format!("rah-session-{current}.md"));
    let deadline = Instant::now() + TIMEOUT;
    let text = loop {
        match std::fs::read_to_string(&download) {
            Ok(text) if !text.is_empty() => break text,
            _ => {
                assert!(
                    Instant::now() < deadline,
                    "no download at {}",
                    download.display()
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    };
    assert!(
        text.starts_with(
            "# Kyoto trip
"
        ),
        "{text}"
    );

    // Delete the current branch from the header.
    page.eval("window.confirm = () => true; byId('session-delete').click(); true")
        .await;
    page.wait(
        "the deleted branch",
        "sessionItems().length === 2 && !shown(byId('session-head')) && byId('input').disabled",
    )
    .await;
    page.eval("true").await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
    let notices = page.eval("notices").await;
    assert_eq!(notices, json!([]));

    // A session with forks cannot be deleted; the reason is shown.
    let original_text = serde_json::to_string(&original).unwrap();
    page.eval(&format!(
        "sessionItems().find((item) => item.dataset.id === {original_text}) \
         .querySelector('[aria-label=\"Delete session\"]').click(); true"
    ))
    .await;
    page.wait(
        "the refusal",
        "shown(byId('notice')) && byId('notice-text').textContent.includes('has forks')",
    )
    .await;
    assert_eq!(page.eval("sessionItems().length").await, 2);

    // Switch the workspace with the folder picker.
    // The workspace is a Git repository with one commit, one modified file
    // and one untracked file.
    let workspace = web.dir.workspace();
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=tester@example.com",
            ])
            .args(args)
            .current_dir(&workspace)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    };
    std::fs::create_dir_all(workspace.join("docs")).unwrap();
    std::fs::write(workspace.join("docs").join("guide.md"), "# Guide\n").unwrap();
    git(&["init", "-q", "-b", "trunk"]);
    git(&["add", "docs"]);
    git(&["commit", "-q", "-m", "add the guide"]);
    std::fs::write(
        workspace.join("docs").join("guide.md"),
        "# Guide\nline two\n",
    )
    .unwrap();
    std::fs::write(workspace.join("notes.txt"), "draft").unwrap();
    std::fs::create_dir_all(web.dir.0.join("other").join(".git")).unwrap();
    page.eval("byId('status-root').click(); true").await;
    page.wait(
        "the folder picker",
        "byId('workspace-dialog').open && byId('ws-path').value.endsWith('workspace')",
    )
    .await;
    page.eval("byId('ws-up').click(); true").await;
    page.wait(
        "the parent folder",
        "[...document.querySelectorAll('#ws-dirs button')].some((b) => \n         b.querySelector('.name').textContent === 'other' && !!b.querySelector('.tag'))",
    )
    .await;
    page.eval(
        "[...document.querySelectorAll('#ws-dirs button')] \n         .find((b) => b.querySelector('.name').textContent === 'other').click(); true",
    )
    .await;
    page.wait(
        "the chosen folder",
        "byId('ws-path').value.endsWith('other')",
    )
    .await;
    page.eval("byId('ws-use').click(); true").await;
    page.wait(
        "the switched workspace",
        "!byId('workspace-dialog').open && byId('status-root').textContent.endsWith('other')",
    )
    .await;
    // The picker remembers it.
    page.eval("byId('status-root').click(); true").await;
    page.wait(
        "the recent folders",
        "shown(byId('ws-recent-block')) && byId('ws-recent').textContent.endsWith('other')",
    )
    .await;
    page.eval("byId('ws-cancel').click(); true").await;
    assert_eq!(page.eval("byId('workspace-dialog').open").await, false);
    page.wait(
        "the saved recent folders",
        "api('/preferences').then((p) => p.recent_workspaces?.[0]?.endsWith('other'))",
    )
    .await;

    // Sessions are grouped by folder: the current one (empty) is open, the
    // old one is listed closed, and the open session of the old folder was
    // closed.
    page.wait(
        "the empty folder",
        "sessionItems().length === 0 && folderItems().length === 2 && \
         folderItems()[0].classList.contains('here') && \
         folderItems()[0].dataset.folder.endsWith('other') && \
         byId('sessions').textContent.includes('No sessions in this folder yet') && \
         byId('input').disabled",
    )
    .await;
    // The workspace panel follows the folder.
    page.wait(
        "the other folder's files",
        "byId('tree-root').textContent === 'other' && treeNames().length === 0",
    )
    .await;
    // Opening the old folder lists its sessions; opening one switches back.
    page.eval(
        "folderItems().find((item) => item.dataset.folder.endsWith('workspace')) \
         .querySelector('button').click(); true",
    )
    .await;
    page.wait("the old folder's sessions", "sessionItems().length === 2")
        .await;
    page.eval("sessionItems()[0].click(); true").await;
    page.wait(
        "the session's own folder",
        "byId('status-root').textContent.endsWith('workspace') && \
         byId('notice-text').textContent.includes('Switched the active folder') && \
         !byId('input').disabled && entries().length > 0",
    )
    .await;
    // Its files, folders first, and a read-only view of one.
    page.wait(
        "the workspace files",
        "byId('tree-root').textContent === 'workspace' && \
         treeNames().join() === 'docs,notes.txt'",
    )
    .await;
    // Git state under the tree: the branch and the changes by folder.
    page.wait(
        "the changed files",
        "!!changeButton('docs/guide.md') && !!changeButton('notes.txt') && \
         changeButton('docs/guide.md').textContent.endsWith('M') && \
         changeButton('notes.txt').textContent.endsWith('U') && \
         byId('git-branch').textContent.includes('trunk')",
    )
    .await;
    page.eval("changeButton('docs/guide.md').click(); true")
        .await;
    page.wait(
        "the diff",
        "document.querySelector('#file-tabs .file-tab.current')?.dataset.key === 'diff:docs/guide.md' && \
         [...byId('file-view').querySelectorAll('.tok-inserted')].some((s) => s.textContent === '+line two')",
    )
    .await;
    page.eval("byId('close-all').click(); true").await;
    // The splitter moves with the keyboard (and the mouse) and is remembered.
    page.eval(
        "byId('pane-splitter').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true })); true",
    )
    .await;
    assert_eq!(
        page.eval("byId('tree-pane').style.getPropertyValue('--tree-ratio')")
            .await,
        "0.550"
    );
    // Remembered by the server, with the rest of the preferences.
    page.wait(
        "the saved split",
        "api('/preferences').then((p) => p.side?.tree_ratio === 0.55)",
    )
    .await;
    page.eval("treeButton('docs').click(); true").await;
    page.wait(
        "the opened folder",
        "treeNames().join() === 'docs,docs/guide.md,notes.txt'",
    )
    .await;
    page.eval("treeButton('docs/guide.md').click(); true").await;
    page.wait(
        "the file view",
        "byId('file-view').querySelector('.lines')?.textContent === '# Guide\\nline two' && \
         byId('file-view').querySelector('.gutter').textContent === '1\\n2' && \
         document.querySelector('#file-tabs .file-tab.current')?.title === 'docs/guide.md'",
    )
    .await;
    // Markdown is highlighted; the text itself is unchanged (checked above).
    assert_eq!(
        page.eval("byId('file-view').querySelector('.lines .tok-heading')?.textContent")
            .await,
        "# Guide"
    );
    // Wrapping shows one row per line, the number beside it, and keeps the
    // text and the highlighting; it is remembered with the preferences.
    page.eval("byId('wrap-files').click(); true").await;
    page.wait(
        "the wrapped file",
        "byId('file-view').querySelector('.code.wrap') && \
         [...byId('file-view').querySelectorAll('.row .num')].map((n) => n.textContent).join() === '1,2' && \
         [...byId('file-view').querySelectorAll('.row .text')].map((t) => t.textContent).join('|') === '# Guide|line two' && \
         byId('file-view').querySelector('.row .tok-heading')?.textContent === '# Guide' && \
         byId('wrap-files').getAttribute('aria-pressed') === 'true' && \
         byId('activity').classList.contains('wrap')",
    )
    .await;
    page.wait(
        "the saved wrap",
        "api('/preferences').then((p) => p.wrap_files === true && p.wrap_activity === true)",
    )
    .await;
    page.eval("byId('wrap-files').click(); true").await;
    page.wait(
        "the unwrapped file",
        "!byId('file-view').querySelector('.code.wrap') && byId('file-view').querySelector('.gutter') && \
         !byId('activity').classList.contains('wrap')",
    )
    .await;
    // The activity log starts folded to its heading; unfolding it is kept.
    assert_eq!(page.eval("shown(byId('activity'))").await, false);
    page.eval("byId('activity-toggle').click(); true").await;
    page.wait(
        "the unfolded activity log",
        "shown(byId('activity')) && shown(byId('view-splitter')) && \
         api('/preferences').then((p) => p.side?.activity_open === true)",
    )
    .await;
    // A theme applies at once and is kept for every window.
    page.eval(
        "state.prefs.theme = 'midnight'; applyTheme(); \
         api('/preferences', { method: 'PUT', body: JSON.stringify(state.prefs) }); true",
    )
    .await;
    assert_eq!(
        page.eval("document.documentElement.dataset.theme").await,
        "midnight"
    );
    page.wait(
        "the theme colours",
        "getComputedStyle(document.body).backgroundColor === 'rgb(13, 18, 32)'",
    )
    .await;
    page.wait(
        "the saved theme",
        "api('/preferences').then((p) => p.theme === 'midnight')",
    )
    .await;
    page.eval("state.prefs.theme = undefined; applyTheme(); true")
        .await;
    assert_eq!(
        page.eval("document.documentElement.dataset.theme === undefined")
            .await,
        true
    );
    page.eval("treeButton('notes.txt').click(); true").await;
    page.wait(
        "a second tab",
        "document.querySelectorAll('#file-tabs .file-tab').length === 2 && \
         byId('file-view').querySelector('.lines')?.textContent === 'draft'",
    )
    .await;
    // A file without a known language stays plain text.
    assert_eq!(
        page.eval("byId('file-view').querySelectorAll('.lines span').length")
            .await,
        0
    );
    page.eval("byId('close-all').click(); true").await;
    page.wait(
        "no open files",
        "document.querySelectorAll('#file-tabs .file-tab').length === 0 && \
         byId('file-view').textContent.includes('read-only')",
    )
    .await;
    // The activity log lists the session's turns in brief.
    assert_eq!(
        page.eval("byId('activity').textContent.includes('turn completed')")
            .await,
        true,
        "{}",
        page.eval("byId('activity').textContent").await
    );

    // Deleting a folder deletes all its sessions, forks first. The current
    // workspace stays listed, empty.
    assert!(page.eval("state.sessions.length").await.as_u64().unwrap() >= 2);
    let folder = page
        .eval(
            "(() => { const box = folderItems().find((item) => item.dataset.folder.endsWith('workspace'))              .getBoundingClientRect(); return [box.x + box.width / 2, box.y + box.height / 2]; })()",
        )
        .await;
    page.call(
        "Input.dispatchMouseEvent",
        json!({ "type": "mouseMoved", "x": folder[0], "y": folder[1] }),
    )
    .await;
    page.screenshot("folder-delete").await;
    page.eval(
        "window.confirm = () => true;          folderItems().find((item) => item.dataset.folder.endsWith('workspace'))          .querySelector('[aria-label=\"Delete folder sessions\"]').click(); true",
    )
    .await;
    page.wait(
        "the emptied folder",
        "state.sessions.length === 0 && sessionItems().length === 0 &&          !shown(byId('session-head')) &&          byId('sessions').textContent.includes('No sessions in this folder yet') &&          !folderItems()[0].querySelector('[aria-label=\"Delete folder sessions\"]')",
    )
    .await;
    assert_eq!(
        page.eval("byId('notice-text').textContent.includes('not deleted')")
            .await,
        false
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn several_repositories_get_a_section_each_in_the_browser() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-repos").await;
    // Two repositories in folders of a workspace that is none itself.
    let workspace = web.dir.workspace();
    let git = |dir: &str, args: &[&str]| {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=Tester",
                "-c",
                "user.email=tester@example.com",
            ])
            .args(args)
            .current_dir(workspace.join(dir))
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    };
    for (dir, branch) in [("api", "main"), ("web", "trunk")] {
        std::fs::create_dir_all(workspace.join(dir).join("src")).unwrap();
        git(dir, &["init", "-q", "-b", branch]);
        std::fs::write(workspace.join(dir).join("src").join("lib.rs"), "one\n").unwrap();
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", "first commit"]);
    }
    std::fs::write(
        workspace.join("api").join("src").join("lib.rs"),
        "one\ntwo\n",
    )
    .unwrap();
    std::fs::write(workspace.join("web").join("notes.txt"), "draft").unwrap();

    let browser = Browser::launch(PathBuf::from(program), "web-browser-repos");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    // The status line shows the branch of one repository, the first at
    // first; a click lists that repository's branches.
    page.wait(
        "the first repository's branch in the status line",
        "byId('status-git').textContent === '⎇ main · api ● 1'",
    )
    .await;
    page.eval("byId('status-git').click(); true").await;
    page.wait(
        "the first repository's branches",
        "!byId('branch-menu').hidden && byId('branch-menu').textContent.includes('main') && \
         !byId('branch-menu').textContent.includes('trunk')",
    )
    .await;
    page.eval("closeBranchMenu(); showPanel('files'); true")
        .await;
    page.wait(
        "a section per repository",
        "[...document.querySelectorAll('#git-body .repo-name')].map((n) => n.textContent).join() === 'api,web' && \
         [...document.querySelectorAll('#git-body summary .git-branch')].map((n) => n.textContent).join() === '⎇ main,⎇ trunk' && \
         !!changeButton('api/src/lib.rs') && !!changeButton('web/notes.txt') && \
         byId('git-branch').textContent === ''",
    )
    .await;
    // A change opens its diff, read in its own repository.
    page.eval("changeButton('api/src/lib.rs').click(); true")
        .await;
    page.wait(
        "the diff",
        "document.querySelector('#file-tabs .file-tab.current')?.dataset.key === 'diff:api/src/lib.rs' && \
         [...byId('file-view').querySelectorAll('.tok-inserted')].some((s) => s.textContent === '+two')",
    )
    .await;
    // A section's branch control lists that repository's branches.
    page.eval("document.querySelectorAll('#git-body summary .git-branch')[1].click(); true")
        .await;
    page.wait(
        "the second repository's branches",
        "!byId('branch-menu').hidden && byId('branch-menu').textContent.includes('trunk') && \
         !byId('branch-menu').textContent.includes('main')",
    )
    .await;
    // A file of the other repository makes the status line follow it.
    page.eval("closeBranchMenu(); changeButton('web/notes.txt').click(); true")
        .await;
    page.wait(
        "the second repository's branch in the status line",
        "byId('status-git').textContent === '⎇ trunk · web ● 1'",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_of_several_folders_is_shown_in_the_browser() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-folders").await;
    let workspace = web.dir.workspace();
    std::fs::write(workspace.join("here.txt"), "the first folder").unwrap();
    // A second folder, a repository on its own branch.
    let other = web.dir.0.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("there.txt"), "the second folder").unwrap();
    let output = Command::new("git")
        .args(["init", "-q", "-b", "side"])
        .current_dir(&other)
        .output()
        .unwrap();
    assert!(output.status.success(), "git init: {output:?}");

    let browser = Browser::launch(PathBuf::from(program), "web-browser-folders");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.wait("the status line", "!!state.workspace").await;
    // The folder picker adds the second folder to the workspace.
    page.eval("byId('tree-add').click(); true").await;
    page.wait(
        "the folder picker",
        "byId('workspace-dialog').open && shown(byId('ws-add')) && byId('ws-path').value.endsWith('workspace')",
    )
    .await;
    let other_path = serde_json::to_string(&other.display().to_string()).unwrap();
    page.eval(&format!(
        "byId('ws-path').value = {other_path}; byId('ws-add').click(); true"
    ))
    .await;
    page.wait(
        "the added folder",
        "!byId('workspace-dialog').open && state.workspaceInfo?.folders?.length === 2",
    )
    .await;
    // The active folder did not move; both folders head the tree.
    page.eval("showPanel('files'); true").await;
    page.wait(
        "both folders in the tree",
        "[...document.querySelectorAll('#tree .folder-name')].map((n) => n.textContent).join() === 'workspace,other' && \
         !!treeButton('here.txt') && byId('status-root').textContent.startsWith('2 folders: ') && \
         byId('status-root').textContent.endsWith('workspace')",
    )
    .await;
    // The other folder's files are read from it, without making it active.
    page.eval("document.querySelector('#tree button[data-folder$=\"other\"]').click(); true")
        .await;
    page.wait("the other folder's files", "!!treeButton('there.txt')")
        .await;
    page.eval("treeButton('there.txt').click(); true").await;
    page.wait(
        "the other folder's file",
        "byId('file-view').textContent.includes('the second folder')",
    )
    .await;
    // Source control names its branch and offers to make it active.
    page.wait(
        "the other folder in source control",
        "!!document.querySelector('#git-body .other-folder') && \
         document.querySelector('#git-body .other-folder').textContent.includes('⎇ side')",
    )
    .await;
    page.eval("document.querySelector('#git-body .other-folder .activate').click(); true")
        .await;
    page.wait(
        "the other folder active",
        "byId('status-root').textContent.endsWith('other') && \
         byId('status-git').textContent.startsWith('⎇ side') && \
         state.workspaceInfo.folders.length === 2 && \
         state.workspaceInfo.folders[1].active",
    )
    .await;
    // The picker remembers the workspace, with its folders.
    page.wait(
        "the remembered workspace",
        "api('/preferences').then((p) => p.recent_workspaces?.[0]?.folders?.length === 2)",
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_change_is_undone_from_the_session_header() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-undo").await;
    let file = web.dir.workspace().join("undo.txt");
    let browser = Browser::launch(PathBuf::from(program), "web-browser-undo");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("byId('new-session').click(); true").await;
    page.wait(
        "the new session",
        "sessionItems().length === 1 && !byId('input').disabled && !shown(byId('session-undo'))",
    )
    .await;
    page.send_message(r#"/call repo.create-file {"path":"undo.txt","content":"u"}"#)
        .await;
    page.wait("the approval card", "shown(approval())").await;
    page.eval("answer('Allow'); true").await;
    page.wait(
        "the Undo button",
        "idle() && shown(byId('session-undo')) && byId('session-undo').title === 'Undo repo.create-file: undo.txt'",
    )
    .await;
    assert!(file.exists());
    page.screenshot("undo-button").await;
    page.eval("byId('session-undo').click(); true").await;
    page.wait(
        "the undo",
        "!shown(byId('session-undo')) && notices.some((n) => n.startsWith('undid `repo.create-file`: removed undo.txt'))",
    )
    .await;
    assert!(!file.exists());
    // `/undo` with nothing left says so.
    page.send_message("/undo").await;
    page.wait(
        "the refusal",
        "notices.includes('There is no file change to undo in this session.')",
    )
    .await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

/// Standard base64 (as DevTools returns screenshots), without a dependency.
fn base64_decode(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut buffer, mut bits) = (0_u32, 0_u32);
    for sextet in text.bytes().filter_map(value) {
        buffer = (buffer << 6) | u32::from(sextet);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    out
}

#[test]
fn base64_decodes_padded_text() {
    assert_eq!(base64_decode("aGVsbG8="), b"hello");
    assert_eq!(base64_decode("UkFI"), b"RAH");
}

/// Clicks the settings tab `tab`.
const TAB: &str = "[...document.querySelectorAll('#settings-tabs button')].find((b) => b.dataset.tab === '{tab}').click(); true";

/// Clicks the settings button labelled `label`.
fn save(label: &str) -> String {
    format!(
        "[...document.querySelectorAll('#settings-actions button')].find((b) => b.textContent === '{label}').click()"
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn settings_change_the_model_assistant_tools_and_loop_in_the_browser() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-settings").await;
    let browser = Browser::launch(PathBuf::from(program), "web-browser-settings");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("byId('new-session').click(); true").await;
    page.wait("the session", "!byId('input').disabled").await;

    // Model: a new label for the mock model takes effect on the next turn.
    page.eval("byId('open-settings').click(); true").await;
    page.wait(
        "the model form",
        "byId('settings-dialog').open && !!document.querySelector('#settings-body select')",
    )
    .await;
    page.screenshot("settings-model").await;
    assert_eq!(
        page.eval("document.querySelector('#settings-body select').value")
            .await,
        "model-mock"
    );
    page.eval(&format!(
        "{{ document.querySelector('#settings-body input').value = 'Z'; {}; true }}",
        save("Save model")
    ))
    .await;
    page.wait(
        "the saved model",
        "byId('settings-message').textContent === \
         'Model saved. Reconnected: Mock (for testing) as model-mock. The next message uses it.'",
    )
    .await;
    // The reconnect is also shown under the conversation, until the next turn.
    assert_eq!(
        page.eval("byId('notice-text').textContent").await,
        "Reconnected: Mock (for testing) as model-mock. The next message uses it."
    );
    page.send_message("/notool hi").await;
    page.wait(
        "the new label",
        "idle() && transcript().includes('[Z] you said: hi')",
    )
    .await;
    assert_eq!(page.eval("byId('notice').hidden").await, true);

    // A model that cannot start is reported, and the old one keeps working.
    page.eval(&format!(
        "{{ const kind = document.querySelector('#settings-body select'); kind.value = 'model-llamacpp'; \
         kind.dispatchEvent(new Event('change')); \
         document.querySelector('#settings-body input').value = 'ftp://example.com'; {}; true }}",
        save("Save model")
    ))
    .await;
    page.wait(
        "the refused model",
        "byId('settings-message').classList.contains('bad') && \
         byId('settings-message').textContent.includes('was kept')",
    )
    .await;
    assert_eq!(
        page.eval("byId('status-model').textContent").await,
        "model: model-mock"
    );

    // Assistant: the system prompt is saved; other options are kept.
    page.eval(&TAB.replace("{tab}", "assistant")).await;
    page.wait(
        "the assistant form",
        "!!document.querySelector('#settings-body textarea')",
    )
    .await;
    page.screenshot("settings-assistant").await;
    page.eval(&format!(
        "{{ document.querySelector('#settings-body textarea').value = 'Be brief.'; {}; true }}",
        save("Save assistant")
    ))
    .await;
    page.wait(
        "the saved assistant",
        "byId('settings-message').textContent === 'Assistant saved.'",
    )
    .await;
    let context = page
        .eval(
            "api('/plugins').then((list) => list.find((p) => p.kind === 'context-default').config)",
        )
        .await;
    assert_eq!(context["system"], "Be brief.", "{context}");
    assert!(context["memory_dir"].is_string(), "{context}");

    // Permissions: one mode for every tool (the base's is `ask`), and a
    // choice per tool under "Advanced"; turn `echo` off; the default choice
    // shows what applies.
    page.eval(&TAB.replace("{tab}", "tools")).await;
    page.wait(
        "the tools form",
        "!!document.querySelector('#settings-body select[data-tool=\"echo\"]')",
    )
    .await;
    assert_eq!(page.eval("byId('permissions-mode').value").await, "ask");
    page.screenshot("settings-tools").await;
    let choice = |tool: &str| {
        format!("document.querySelector('#settings-body select[data-tool=\"{tool}\"]')")
    };
    assert_eq!(
        page.eval(&format!("{}.options[0].textContent", choice("echo")))
            .await,
        "Default (allowed)"
    );
    assert_eq!(
        page.eval(&format!("{}.options[0].textContent", choice("repo.patch")))
            .await,
        "Default (asks first)"
    );
    page.eval(&format!(
        "{{ {}.value = 'deny'; {}; true }}",
        choice("echo"),
        save("Save permissions")
    ))
    .await;
    page.wait(
        "the saved tools",
        "byId('settings-message').textContent === 'Permissions saved.'",
    )
    .await;
    page.send_message("hello").await;
    page.wait(
        "the turned-off tool",
        "idle() && transcript().includes('turned off in the policy')",
    )
    .await;
    let policy = page
        .eval("api('/plugins').then((list) => list.find((p) => p.kind === 'policy-authority').config)")
        .await;
    assert_eq!(policy["decisions"]["echo"], "deny", "{policy}");
    assert_eq!(
        policy["tools"]["repo.*"], "read",
        "assignments are kept: {policy}"
    );

    // A mode is the `permissions` key of rah.toml.
    page.eval(&TAB.replace("{tab}", "tools")).await;
    page.wait("the permissions form", "!!byId('permissions-mode')")
        .await;
    page.eval(&format!(
        "{{ byId('permissions-mode').value = 'auto-edit'; {}; true }}",
        save("Save permissions")
    ))
    .await;
    page.wait(
        "the saved mode",
        "byId('settings-message').textContent === 'Permissions saved.'",
    )
    .await;
    let policy = page
        .eval("api('/plugins').then((list) => list.find((p) => p.kind === 'policy-authority').config)")
        .await;
    assert_eq!(
        policy["allow"],
        json!(["none", "read", "write"]),
        "{policy}"
    );
    assert_eq!(policy["ask"], json!(["execute"]), "{policy}");
    let file = std::fs::read_to_string(common::config_file(&web.dir)).unwrap();
    assert!(file.contains("permissions = \"auto-edit\""), "{file}");
    assert!(file.contains("echo = \"deny\""), "{file}");

    // Features: a switch applies at once and is kept in rah.toml.
    page.eval(&TAB.replace("{tab}", "features")).await;
    page.wait(
        "the features form",
        "!!document.querySelector('#settings-body input[data-feature=\"todo\"]')",
    )
    .await;
    assert_eq!(
        page.eval(
            "document.querySelector('#settings-body input[data-feature=\"web-search\"]') === null"
        )
        .await,
        true,
        "web search keeps its switch in Permissions"
    );
    page.eval(
        "document.querySelector('#settings-body input[data-feature=\"todo\"]').click(); true",
    )
    .await;
    page.wait(
        "the switched feature",
        "byId('settings-message').textContent === 'todo is on.'",
    )
    .await;
    let file = std::fs::read_to_string(common::config_file(&web.dir)).unwrap();
    assert!(file.contains("\"todo\""), "{file}");

    // Web search: one switch mounts the tools and makes the policy ask.
    page.eval(&TAB.replace("{tab}", "tools")).await;
    page.wait(
        "the web search switch",
        "!!byId('web-search-switch') && !byId('web-search-switch').checked",
    )
    .await;
    page.eval("byId('web-search-switch').click(); true").await;
    page.wait(
        "web search on",
        "byId('settings-message').textContent.startsWith('Web search is on.') && \
         byId('web-search-switch').checked && \
         !!document.querySelector('#settings-body select[data-tool=\"web.search\"]')",
    )
    .await;
    page.screenshot("settings-web-search").await;
    let policy = page
        .eval("api('/plugins').then((list) => list.find((p) => p.kind === 'policy-authority').config)")
        .await;
    assert_eq!(policy["decisions"]["web.*"], "ask", "{policy}");
    assert_eq!(policy["decisions"]["echo"], "deny", "{policy}");

    // Display: replies are formatted Markdown by default; the source is
    // untrusted, so HTML stays text and only web links become links.
    page.send_message(
        "/notool intro\n# Title\n**bold** and `code` [ok](https://example.com/a) [bad](javascript:alert(1)) <img src=x onerror=alert(1)>\n\n- one\n- [x] done\n\n| a | b |\n|---|--:|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```",
    )
    .await;
    let reply = "[...document.querySelectorAll('.entry.assistant')].pop()";
    page.wait(
        "the formatted reply",
        &format!("idle() && !!{reply}?.querySelector('.md h1')"),
    )
    .await;
    page.eval("byId('settings-close').click(); true").await;
    page.screenshot("markdown-formatted").await;
    page.eval("byId('open-settings').click(); true").await;
    let formatted = page
        .eval(&format!(
            "(() => {{ const r = {reply}; return {{ strong: r.querySelector('strong')?.textContent, \
             code: r.querySelector('p code')?.textContent, links: [...r.querySelectorAll('a')].map((a) => a.href), \
             images: r.querySelectorAll('img').length, text: r.textContent, items: r.querySelectorAll('li').length, \
             checked: r.querySelector('li.task input')?.checked, cells: r.querySelectorAll('td').length, \
             align: r.querySelector('td:last-child')?.style.textAlign, \
             highlighted: r.querySelectorAll('pre.md-code [class^=tok-]').length }}; }})()"
        ))
        .await;
    assert_eq!(formatted["strong"], "bold", "{formatted}");
    assert_eq!(formatted["code"], "code", "{formatted}");
    assert_eq!(
        formatted["links"],
        json!(["https://example.com/a"]),
        "{formatted}"
    );
    assert_eq!(formatted["images"], 0, "{formatted}");
    // The unsafe link keeps only its label, as text.
    assert!(
        formatted["text"].as_str().unwrap().contains("ok bad <img"),
        "{formatted}"
    );
    assert!(
        formatted["text"]
            .as_str()
            .unwrap()
            .contains("<img src=x onerror=alert(1)>"),
        "{formatted}"
    );
    assert_eq!(formatted["items"], 2, "{formatted}");
    assert_eq!(formatted["checked"], true, "{formatted}");
    assert_eq!(formatted["cells"], 2, "{formatted}");
    assert_eq!(formatted["align"], "right", "{formatted}");
    assert!(
        formatted["highlighted"].as_u64().unwrap() > 0,
        "{formatted}"
    );

    // Display is a section of the Features & display tab.
    page.eval(&TAB.replace("{tab}", "features")).await;
    page.wait(
        "the display form",
        "!!byId('raw-markdown') && !byId('raw-markdown').checked",
    )
    .await;
    page.eval("byId('raw-markdown').click(); true").await;
    page.wait(
        "the raw reply",
        &format!(
            "byId('settings-message').textContent === 'Replies are shown as Markdown source.' && \
             byId('raw-markdown').checked && !byId('raw-markdown').disabled && \
             !{reply}.querySelector('.md') && {reply}.querySelector('pre').textContent.includes('**bold**')"
        ),
    )
    .await;
    let preferences = page.eval("api('/preferences')").await;
    assert_eq!(preferences["raw_markdown"], true, "{preferences}");
    page.eval("byId('raw-markdown').click(); true").await;
    page.wait(
        "the formatted reply again",
        &format!("!!{reply}.querySelector('.md strong')"),
    )
    .await;

    // Loop: switch to plan-and-execute.
    // It is a section of the Model tab.
    page.eval(&TAB.replace("{tab}", "model")).await;
    page.wait("the loop form", "!!byId('loop-kind')").await;
    page.eval(&format!(
        "{{ const kind = byId('loop-kind'); kind.value = 'loop-plan'; \
         kind.dispatchEvent(new Event('change')); {}; true }}",
        save("Save loop")
    ))
    .await;
    page.wait(
        "the new loop",
        "byId('settings-message').textContent === \
         'Loop saved. Switched to loop loop-plan. The next message uses it.' && \
         byId('status-agent').textContent.includes('loop: loop-plan')",
    )
    .await;
    // The switch is also shown under the conversation. The save shows it
    // after the status refresh, so a status poll during the save cannot
    // replace it.
    assert_eq!(
        page.eval("byId('notice-text').textContent").await,
        "Switched to loop loop-plan. The next message uses it."
    );
    let notices = page.eval("notices").await;
    assert!(
        !notices
            .as_array()
            .unwrap()
            .iter()
            .any(|notice| notice.as_str().unwrap_or_default().contains("loop-react")),
        "no switch back to the old loop: {notices:#}"
    );
    page.eval("byId('settings-close').click(); true").await;
    assert_eq!(page.eval("byId('settings-dialog').open").await, false);

    page.eval("true").await;
    // The refused model configuration answers 400; that network error is
    // expected.
    let unexpected: Vec<_> = page
        .errors()
        .into_iter()
        .filter(|event| {
            let entry = &event["params"]["entry"];
            let refused = entry["text"].as_str().unwrap_or_default().contains("400")
                && entry["url"]
                    .as_str()
                    .unwrap_or_default()
                    .ends_with("/api/settings/model");
            !refused
        })
        .collect();
    assert!(unexpected.is_empty(), "page errors: {unexpected:#?}");
}

/// Which column holds each panel, and the tab labels of both columns.
const PLACES: &str = r#"({
  sessions: byId('sessions-panel').parentElement.id,
  files: byId('files-view').parentElement.id,
  plugins: byId('plugins-panel').parentElement.id,
  left: [...byId('left-tabs').querySelectorAll('[data-view]')].map((b) => b.textContent),
  right: [...byId('side-tabs').querySelectorAll('[data-view]')].map((b) => b.textContent),
  leftTabs: shown(byId('left-tabs')),
  sideButton: shown(byId('toggle-side')) && byId('toggle-side').getAttribute('aria-label'),
})"#;

#[tokio::test(flavor = "multi_thread")]
async fn a_ui_layout_plugin_moves_panels_and_the_standard_layout_is_kept_without_one() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-layout").await;
    let browser = Browser::launch(PathBuf::from(program), "web-browser-layout");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    assert_eq!(
        page.eval(PLACES).await,
        json!({
            "sessions": "left", "files": "side", "plugins": "side",
            "left": ["Sessions"], "right": ["Workspace", "Plugins", "Usage", "My plugins"],
            "leftTabs": false, "sideButton": "Workspace",
        })
    );
    assert_eq!(page.eval("shown(byId('sessions-panel'))").await, true);

    let id = rah_api::PluginId::new("my-layout");
    common::mount_layout(
        &web._kernel,
        &id,
        json!({ "left": ["sessions", "plugins"], "right": ["files"] }),
    )
    .await;
    // The page applies the new layout by itself, without a reload.
    page.wait(
        "the pushed layout",
        "byId('plugins-panel').parentElement.id === 'left'",
    )
    .await;
    assert_eq!(
        page.eval(PLACES).await,
        json!({
            "sessions": "left", "files": "side", "plugins": "left",
            "left": ["Sessions", "Plugins"], "right": ["Workspace", "Usage", "My plugins"],
            "leftTabs": true, "sideButton": "Workspace",
        })
    );
    // The left column starts on its first panel; its Plugins tab lists them.
    assert_eq!(page.eval("shown(byId('sessions-panel'))").await, true);
    page.eval("[...byId('left-tabs').querySelectorAll('[data-view]')][1].click(); true")
        .await;
    page.wait(
        "the plugins in the left column",
        "shown(byId('plugins-panel')) && !shown(byId('sessions-panel')) && byId('plugins').rows.length > 0",
    )
    .await;
    page.screenshot("layout-plugins-left").await;

    // Removing the plugin brings the standard layout back, without a reload.
    web._kernel.unmount(&id).await.unwrap();
    page.wait(
        "the standard layout",
        "byId('plugins-panel').parentElement.id === 'side' && byId('left-tabs').hidden",
    )
    .await;

    // Adding and configuring `ui-layout-config` as the Plugins panel does:
    // each change applies in every window at once.
    page.eval(
        "api('/plugins', { method: 'POST', body: JSON.stringify({ kind: 'ui-layout-config', \
         config: { left: ['plugins'], right: ['sessions', 'files'] } }) }); true",
    )
    .await;
    page.wait(
        "the added layout plugin",
        "byId('plugins-panel').parentElement.id === 'left' && byId('sessions-panel').parentElement.id === 'side'",
    )
    .await;
    page.eval(
        "api('/plugins/ui-layout-config/config', { method: 'PUT', body: JSON.stringify({ \
         config: { left: ['files', 'sessions'], right: ['plugins'] } }) }); true",
    )
    .await;
    page.wait(
        "the configured layout",
        "byId('files-view').parentElement.id === 'left' && byId('sessions-panel').parentElement.id === 'left' \
         && byId('plugins-panel').parentElement.id === 'side'",
    )
    .await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plugin_view_is_a_tab_that_follows_turns_and_runs_its_actions() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    // The tests turn `telemetry` and `plugin-builder` on, with their views.
    let web = start("web-browser-views").await;
    let browser = Browser::launch(PathBuf::from(program), "web-browser-views");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    assert_eq!(
        page.eval(
            "[...byId('side-tabs').querySelectorAll('[data-view]')].map((b) => b.textContent)"
        )
        .await,
        json!(["Workspace", "Plugins", "Usage", "My plugins"])
    );
    const TURNS: &str = "[...document.querySelectorAll('.view-values dd')][0]?.textContent";
    page.eval("showPanel('telemetry-log/usage'); true").await;
    page.wait(
        "the usage view",
        &format!("{TURNS} === '0 (0 completed, 0 failed, 0 cancelled)'"),
    )
    .await;

    page.eval("byId('new-session').click(); true").await;
    page.wait(
        "the new session",
        "sessionItems().length === 1 && !byId('input').disabled",
    )
    .await;
    page.send_message("hello").await;
    page.wait(
        "the view to follow the turn",
        &format!("idle() && {TURNS} === '1 (1 completed, 0 failed, 0 cancelled)'"),
    )
    .await;
    page.screenshot("usage-view").await;

    page.eval("[...document.querySelectorAll('.view-panel button')].find((b) => b.textContent === 'Reset').click(); true")
        .await;
    page.wait(
        "the reset",
        &format!(
            "{TURNS} === '0 (0 completed, 0 failed, 0 cancelled)' && \
             document.querySelector('.view-message').textContent === 'Usage sums start again from now.'"
        ),
    )
    .await;
    // A turn this window does not follow: only the provider's `UiChanged`
    // tells the page.
    let handle = web._kernel.handle();
    let other = common::new_session(&handle).await;
    common::turn(&handle, &other, "elsewhere").await;
    page.wait(
        "the pushed change",
        &format!("{TURNS} === '1 (1 completed, 0 failed, 0 cancelled)'"),
    )
    .await;

    // Adding a layout plugin lists every panel it can place.
    page.eval(
        "showPanel('plugins'); byId('add-plugin').click();          byId('editor-kind').value = 'ui-layout-config';          byId('editor-kind').dispatchEvent(new Event('change')); true",
    )
    .await;
    let panels = page
        .wait(
            "the panels you can place",
            "(() => { const lines = [...document.querySelectorAll('.panel-ids div')].map((line) => line.textContent);              return lines.length ? lines : null; })()",
        )
        .await;
    assert_eq!(
        panels,
        json!([
            "sessions Sessions (built-in; now left)",
            "files Workspace (built-in; now right)",
            "plugins Plugins (built-in; now right)",
            "telemetry-log/usage Usage (view of telemetry-log; now right)",
            "plugin-builder/plugins My plugins (view of plugin-builder; now right)",
        ])
    );
    page.screenshot("layout-panels").await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shown_view_follows_the_session_that_is_opened() {
    use rah_api::tools::{Invocation, ToolInvoker};
    use rah_protocol::{ToolCall, ToolCallId, ToolInput, ToolName};
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start_with("web-browser-view-session", |distribution, _| {
        distribution.enable("todo").unwrap();
    })
    .await;
    let handle = web._kernel.handle();
    let planned = common::new_session(&handle).await;
    let empty = common::new_session(&handle).await;
    let outcome = ToolInvoker::new(handle.clone())
        .invoke(
            Invocation {
                session: planned.clone(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("todo.write"),
                    input: ToolInput(
                        json!({ "todos": [{ "content": "fix", "status": "in_progress" }] }),
                    ),
                },
                cancel: rah_api::CancellationToken::new(),
            },
            || {},
        )
        .await;
    assert!(!outcome.output.is_error, "{:?}", outcome.output);

    let browser = Browser::launch(PathBuf::from(program), "web-browser-view-session");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("showPanel('todo/list'); true").await;
    const BODY: &str = "views.get('todo/list').body.textContent";
    // Each switch renders the view for the session just opened.
    for (session, shown) in [
        (&planned, "[>] fix"),
        (&empty, "No to-do items"),
        (&planned, "[>] fix"),
    ] {
        page.eval(&format!("openSession('{session}'); true")).await;
        page.wait(
            "the view of the opened session",
            &format!("state.current === '{session}' && {BODY}.includes('{shown}')"),
        )
        .await;
    }
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

/// A Python 3 interpreter on PATH, if any.
fn python() -> Option<&'static str> {
    ["python3", "python"].into_iter().find(|program| {
        Command::new(program)
            .arg("--version")
            .output()
            .is_ok_and(|output| {
                output.status.success()
                    && String::from_utf8_lossy(&output.stdout).starts_with("Python 3")
            })
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_python_view_takes_a_form_and_a_button_in_the_browser() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let Some(python) = python() else {
        eprintln!("skipped: no Python 3 interpreter on PATH");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-python-view").await;
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/process-plugins/todo_board.py");
    let config = json!({ "program": python, "args": [script.display().to_string()] });
    let plugin =
        rah_plugin_host_process::ProcessPlugin::describe(rah_api::PluginId::new("todo"), &config)
            .await
            .unwrap();
    web._kernel
        .mount(Arc::new(plugin), rah_api::MountOptions::with_config(config))
        .await
        .unwrap();
    let browser = Browser::launch(PathBuf::from(program), "web-browser-python-view");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("showPanel('todo/board'); true").await;
    const BOARD: &str = "document.querySelector('[aria-label=\"To-do\"]')";
    page.wait(
        "the board",
        &format!("{BOARD}.querySelector('.view-body p')?.textContent === '0 item(s).'"),
    )
    .await;
    // An empty item is refused by the plugin and the message says why.
    page.eval(&format!(
        "{BOARD}.querySelector('.view-form button').click(); true"
    ))
    .await;
    page.wait(
        "the refusal",
        &format!(
            "{BOARD}.querySelector('.view-message.bad')?.textContent.includes('type an item first')"
        ),
    )
    .await;
    page.eval(&format!(
        "{BOARD}.querySelector('.view-form input').value = 'water the plants'; \
         {BOARD}.querySelector('.view-form button').click(); true"
    ))
    .await;
    page.wait(
        "the added item",
        &format!(
            "[...{BOARD}.querySelectorAll('.view-list li')].map((li) => li.textContent).join() === 'water the plants' \
             && {BOARD}.querySelector('.view-message').textContent === 'Added “water the plants”.'"
        ),
    )
    .await;
    page.screenshot("python-view").await;
    page.eval(&format!(
        "[...{BOARD}.querySelectorAll('button')].find((b) => b.textContent === 'Clear').click(); true"
    ))
    .await;
    page.wait(
        "the cleared board",
        &format!("{BOARD}.querySelector('.view-body p')?.textContent === '0 item(s).'"),
    )
    .await;
    // The refused empty item is a 400, which the browser logs; nothing else.
    let errors: Vec<_> = page
        .errors()
        .into_iter()
        .filter(|error| {
            let entry = &error["params"]["entry"];
            !(entry["url"]
                .as_str()
                .is_some_and(|url| url.ends_with("/actions/add"))
                && entry["text"]
                    .as_str()
                    .is_some_and(|text| text.contains("400")))
        })
        .collect();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn personal_plugins_are_listed_apart_in_their_own_color() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let node_found = Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !node_found {
        eprintln!("skipped: no Node.js on PATH");
        return;
    }
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-personal").await;
    // A personal plugin as `plugin-builder` saves it, mounted the way
    // `mount.toml` says, through the admin service like the Plugins panel.
    let plugin_dir = web.dir.0.join("greeter");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let runtime = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/plugin-builder/src/runtime/rah_plugin.mjs");
    std::fs::copy(runtime, plugin_dir.join("rah_plugin.mjs")).unwrap();
    std::fs::write(
        plugin_dir.join("manifest.json"),
        json!({
            "version": "0.1.0",
            "provides": ["tools"],
            "tools": [{
                "name": "greeter.hello",
                "description": "Greets.",
                "input_schema": { "type": "object" },
                "permission": "none"
            }]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        plugin_dir.join("plugin.mjs"),
        "import { Plugin } from \"./rah_plugin.mjs\";\n\
         const plugin = new Plugin(import.meta.url);\n\
         plugin.tool(\"greeter.hello\", async () => \"hello\");\n\
         plugin.run();\n",
    )
    .unwrap();
    let admin = web._kernel.handle().get(rah_api::admin::ADMIN).unwrap();
    admin
        .mount(rah_api::admin::MountRequest {
            kind: "process".into(),
            id: Some("greeter".into()),
            config: json!({
                "program": "node",
                "args": [plugin_dir.join("plugin.mjs").display().to_string()],
                "cwd": plugin_dir.display().to_string(),
                "personal": true
            }),
            ..Default::default()
        })
        .await
        .unwrap();

    let browser = Browser::launch(PathBuf::from(program), "web-browser-personal");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("byId('status-agent').click(); true").await;
    page.wait(
        "the personal plugin",
        "shown(byId('plugins-panel')) && !!rowOf('greeter')",
    )
    .await;
    // The distribution's plugins come first, then the personal group.
    let layout = page
        .eval(
            "(() => { const rows = [...document.querySelectorAll('#plugins tr')]; \
             const group = (label) => rows.findIndex((row) => row.classList.contains('group') && row.textContent === label); \
             return [group('RAH plugins'), group('Personal plugins'), rows.indexOf(rowOf('policy-authority')), rows.indexOf(rowOf('greeter')), rows.length - 1].join(); })()",
        )
        .await;
    let indices: Vec<i64> = layout
        .as_str()
        .unwrap()
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    let [builtin, personal, policy, greeter, last] = indices[..] else {
        panic!("{layout}");
    };
    assert_eq!(builtin, 0, "{layout}");
    assert!(
        policy > builtin && personal > policy && greeter > personal,
        "{layout}"
    );
    assert_eq!(greeter, last, "{layout}");
    // Tagged, and in the theme's personal color; RAH's own rows are not.
    assert_eq!(
        page.eval("rowOf('greeter').querySelector('.personal-tag').textContent")
            .await,
        "Personal"
    );
    let colors = page
        .eval(
            "(() => { const personal = getComputedStyle(document.documentElement).getPropertyValue('--personal').trim(); \
             const probe = document.createElement('span'); probe.style.color = personal; document.body.append(probe); \
             const expected = getComputedStyle(probe).color; probe.remove(); \
             return [expected, getComputedStyle(rowOf('greeter').cells[0]).color, getComputedStyle(rowOf('policy-authority').cells[0]).color].join('|'); })()",
        )
        .await;
    let colors: Vec<&str> = colors.as_str().unwrap().split('|').collect();
    assert_eq!(colors[0], colors[1], "{colors:?}");
    assert_ne!(colors[0], colors[2], "{colors:?}");
    assert!(
        page.eval("!rowOf('policy-authority').querySelector('.personal-tag')")
            .await
            == true
    );
    page.screenshot("personal-plugins").await;
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_approval_with_large_input_keeps_its_buttons_in_view() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-browser-large-approval").await;
    let file = web.dir.workspace().join("large.txt");
    let browser = Browser::launch(PathBuf::from(program), "web-browser-large-approval");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    page.eval("byId('new-session').click(); true").await;
    page.wait(
        "the new session",
        "sessionItems().length === 1 && !byId('input').disabled",
    )
    .await;
    // Far more input than the window is tall, like a whole plugin.
    let content: String = (0..400).map(|n| format!("line {n}\n")).collect();
    let call = format!(
        "/call repo.create-file {}",
        json!({ "path": "large.txt", "content": content })
    );
    page.send_message(&call).await;
    page.wait("the approval card", "shown(approval())").await;
    // Every button is inside the window and is what a click there hits.
    let reachable = page
        .eval(
            "[...approval().querySelectorAll('.buttons button')].every((button) => { \
               const box = button.getBoundingClientRect(); \
               return box.bottom <= innerHeight && box.right <= innerWidth && box.top >= 0 \
                 && document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2) === button; })",
        )
        .await;
    assert_eq!(reachable, true);
    // The input scrolls inside the card.
    assert_eq!(
        page.eval("(() => { const pre = approval().querySelector('pre'); return pre.scrollHeight > pre.clientHeight; })()")
            .await,
        true
    );
    page.screenshot("large-approval").await;
    page.eval("answer('Allow'); true").await;
    page.wait("the written file", "idle() && !approval()").await;
    assert_eq!(std::fs::read_to_string(&file).unwrap(), content);
    let errors = page.errors();
    assert!(errors.is_empty(), "page errors: {errors:#?}");
}

#[tokio::test]
async fn user_message_navigation_tracks_scroll_and_session_changes() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        return;
    };
    let _one = ONE_BROWSER.lock().await;
    let web = start("web-message-navigation").await;
    let browser = Browser::launch(PathBuf::from(program), "web-message-navigation");
    let mut page = browser.page().await;
    page.call("Page.navigate", json!({"url":web.url})).await;
    page.wait("connected", "connected()").await;
    page.eval("byId('new-session').click(); true").await;
    page.wait(
        "empty session",
        "!!state.current && !byId('input').disabled",
    )
    .await;
    assert_eq!(page.eval("byId('message-nav').hidden").await, true);
    let original = page.eval("state.current").await;
    for index in 1..=4 {
        let topic = [
            "Find an older conversation by its contents",
            "Jump to a question without scrolling through every reply",
            "Measure search performance with a larger history",
            "Make the conversation map clearer and easier to use",
        ][index - 1];
        let message = format!(
            "{topic}. {} <img src=x onerror=alert(1)>",
            "Additional detail for this question. ".repeat(45)
        );
        page.send_message(&message).await;
        page.wait(
            "navigation includes the completed turn",
            &format!("idle() && byId('message-anchors').children.length === {index}"),
        )
        .await;
    }
    assert_eq!(
        page.eval("byId('message-anchors').querySelectorAll('img').length")
            .await,
        0
    );
    assert_eq!(
        page.eval("byId('message-anchors').firstChild.title.includes('<img src=x')")
            .await,
        true
    );
    page.eval("byId('message-anchors').firstChild.focus(); true")
        .await;
    page.call("Input.dispatchKeyEvent", json!({"type":"keyDown", "key":"Enter", "code":"Enter", "windowsVirtualKeyCode":13, "text":"\r"})).await;
    page.call(
        "Input.dispatchKeyEvent",
        json!({"type":"keyUp", "key":"Enter", "code":"Enter", "windowsVirtualKeyCode":13}),
    )
    .await;
    page.wait(
        "first message selected",
        "byId('message-anchors').firstChild.getAttribute('aria-current') === 'location'",
    )
    .await;
    assert_eq!(page.eval("Math.abs(document.querySelector('#transcript .user').getBoundingClientRect().top - byId('transcript').getBoundingClientRect().top - 12) < 2").await, true);
    assert_eq!(
        page.eval(
            "!byId('message-nav-count') && byId('message-nav').getBoundingClientRect().height <= 35 && getComputedStyle(byId('message-nav')).flexDirection === 'row'"
        )
        .await,
        true
    );
    assert_eq!(page.eval("{ const text = document.querySelector('.message-anchor-text'); getComputedStyle(text).whiteSpace === 'nowrap' && getComputedStyle(text).textOverflow === 'ellipsis' && text.scrollWidth > text.clientWidth && text.getBoundingClientRect().height <= 18 }").await, true);
    assert_eq!(page.eval("{ const cards = byId('message-anchors').children; getComputedStyle(cards[0]).getPropertyValue('--question-color') !== getComputedStyle(cards[1]).getPropertyValue('--question-color') }").await, true);
    assert_eq!(page.eval("getComputedStyle(byId('message-anchors').firstChild).backgroundColor !== getComputedStyle(document.querySelector('#transcript .user')).backgroundColor").await, true);
    assert_eq!(page.eval("byId('message-nav-prev').disabled").await, true);
    page.eval("byId('message-nav-next').click(); true").await;
    page.wait(
        "next question selected",
        "byId('message-anchors').children[1].hasAttribute('aria-current')",
    )
    .await;
    page.eval("byId('message-nav-prev').click(); true").await;
    page.wait(
        "previous question selected",
        "byId('message-anchors').firstChild.hasAttribute('aria-current')",
    )
    .await;

    page.eval("byId('message-anchors').lastChild.click(); true")
        .await;
    page.wait(
        "last message selected",
        "byId('message-anchors').lastChild.getAttribute('aria-current') === 'location'",
    )
    .await;
    assert_eq!(page.eval("byId('message-nav-next').disabled").await, true);
    page.eval("byId('transcript').scrollTop = 0; true").await;
    page.wait(
        "manual scroll updates selection",
        "byId('message-anchors').firstChild.getAttribute('aria-current') === 'location'",
    )
    .await;
    page.screenshot("message-navigation-desktop").await;
    page.eval("document.documentElement.dataset.theme = 'dark'; true")
        .await;
    page.screenshot("message-navigation-dark").await;
    page.eval("document.documentElement.dataset.theme = 'light'; true")
        .await;

    page.call(
        "Emulation.setDeviceMetricsOverride",
        json!({"width":480,"height":800,"deviceScaleFactor":1,"mobile":false}),
    )
    .await;
    page.eval("hideSide(); true").await;
    page.wait("navigation is visible on a narrow screen", "byId('message-nav').getBoundingClientRect().height > 0 && byId('message-anchors').clientWidth > 0").await;
    assert_eq!(
        page.eval("byId('message-anchors').scrollWidth > byId('message-anchors').clientWidth")
            .await,
        true
    );
    assert_eq!(
        page.eval("document.documentElement.scrollWidth <= innerWidth")
            .await,
        true
    );
    page.eval("byId('message-anchors').lastChild.focus(); byId('message-anchors').lastChild.click(); true").await;
    page.wait("last summary visible on a narrow screen", "byId('message-anchors').lastChild.getBoundingClientRect().right <= byId('message-anchors').getBoundingClientRect().right + 1").await;
    assert_eq!(page.eval("byId('message-nav').getBoundingClientRect().height <= 35 && document.querySelector('.message-anchor-text').getBoundingClientRect().height <= 18").await, true);
    page.screenshot("message-navigation-narrow").await;
    page.eval("byId('new-session').click(); true").await;
    page.wait(
        "new session clears navigation",
        "byId('message-nav').hidden && !byId('input').disabled",
    )
    .await;
    page.eval(&format!("openSession({original}).then(() => true)"))
        .await;
    page.wait(
        "restored session restores navigation",
        "byId('message-anchors').children.length === 4",
    )
    .await;
    page.eval("closeSession(); true").await;
    assert_eq!(
        page.eval("byId('message-nav').hidden && byId('message-anchors').children.length === 0")
            .await,
        true
    );
    assert!(page.errors().is_empty(), "{:?}", page.errors());
    web._kernel.shutdown().await.unwrap();
}
