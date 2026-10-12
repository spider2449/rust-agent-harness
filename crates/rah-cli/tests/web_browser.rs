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

use common::{TempDir, headless};
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
    _profile: TempDir,
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
            _profile: profile,
        }
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
window.rowAction = (id, label) => [...rowOf(id).querySelectorAll('button')]
  .find((button) => button.textContent === label).click();
window.approval = () => document.querySelector('#approvals .approval');
window.answer = (label) => [...approval().querySelectorAll('button')]
  .find((button) => button.textContent === label).click();
window.idle = () => !byId('send').disabled && !shown(byId('cancel')) && !shown(byId('live'));
window.sessionItems = () => [...document.querySelectorAll('#sessions li.session')];
window.headTitle = () => byId('session-title').textContent;
window.metaAction = (label) => [...document.querySelectorAll('#transcript .meta button')]
  .filter((button) => button.textContent === label);
// Every notice the page shows, even one that is replaced right away.
window.notices = [];
document.addEventListener('DOMContentLoaded', () => {
  new MutationObserver(() => {
    const text = byId('notice').textContent;
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
  notice: document.getElementById('notice')?.textContent,
  connection: document.getElementById('connection')?.textContent,
  plugins: [...document.querySelectorAll('#plugins tr')].map((row) => [...row.cells].map((cell) => cell.textContent)),
  approvals: document.getElementById('approvals')?.textContent,
  live: document.getElementById('live')?.textContent,
  transcript: [...document.querySelectorAll('#transcript .entry')].map((entry) => entry.textContent),
  session: { current: state.current, running: state.running, starting: !!state.starting, workspace: state.workspace, title: document.getElementById('session-title')?.textContent },
  sessions: [...document.querySelectorAll('#sessions li.session')].map((item) => item.dataset.id + ' ' + item.textContent),
}))()"#;

struct Web {
    _kernel: Kernel,
    url: String,
    dir: TempDir,
}

async fn start(name: &str) -> Web {
    let dir = TempDir::new(name);
    let mut distribution = headless(&dir);
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
    assert_eq!(page.eval("byId('guard').textContent").await, "guarded");

    // The status line: workspace, Git state (not a repository here) and
    // the providers the next turn uses.
    page.wait(
        "the status line",
        "shown(byId('status')) && byId('status-model').textContent === 'model: model-mock'",
    )
    .await;
    let status = page
        .eval(
            "['status-root', 'status-git', 'status-loop', 'status-policy'] \
             .map((id) => byId(id).textContent)",
        )
        .await;
    assert!(
        status[0].as_str().unwrap().ends_with("workspace"),
        "{status}"
    );
    assert_eq!(
        status.as_array().unwrap()[1..],
        [
            json!("no git repository"),
            json!("loop: loop-react"),
            json!("guarded")
        ],
        "{status}"
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
        json!(["you", "tool call · echo", "tool result", "assistant"])
    );
    assert_eq!(page.eval("byId('input').value").await, "");

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
    page.eval("byId('toggle-plugins').click(); true").await;
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
    assert_eq!(
        page.eval("rowOf('distribution').querySelectorAll('button').length")
            .await,
        0
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
    page.send_message("/notool after replace").await;
    page.wait(
        "the replacement reply",
        "transcript().includes('[B] you said: after replace') && idle()",
    )
    .await;

    // Add a plugin, then remove it after the confirmation.
    page.eval("byId('add-plugin').click(); true").await;
    page.wait("the add editor", "shown(byId('editor-id-row'))")
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
        "shown(byId('notice')) && byId('notice').textContent.includes('not valid JSON')",
    )
    .await;
    page.eval("byId('editor-cancel').click(); true").await;
    let notices = page.eval("notices").await;
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

    // A failed startup request stays reported after the socket connects.
    page.eval("window.confirm = () => true; rowAction('session-jsonl', 'Remove'); true")
        .await;
    page.wait("the removed session plugin", "!rowOf('session-jsonl')")
        .await;
    page.call("Page.reload", json!({})).await;
    page.wait(
        "the startup error after connecting",
        "connected() && shown(byId('notice')) && \
         byId('notice').textContent.includes('no session service is mounted')",
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
    // The buttons' handler, awaited so the downloads finish before the delete.
    assert_eq!(
        page.eval("shown(byId('export-md')) && shown(byId('export-json'))")
            .await,
        true
    );
    page.eval("Promise.all([exportSession('md'), exportSession('json')]).then(() => true)")
        .await;

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
        "shown(byId('notice')) && byId('notice').textContent.includes('has forks')",
    )
    .await;
    assert_eq!(page.eval("sessionItems().length").await, 2);

    // Switch the workspace with the folder picker.
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

    // Sessions belong to their folder: the new folder has none, and the open
    // session of the old folder was closed.
    page.wait(
        "the empty folder",
        "sessionItems().length === 0 && \
         byId('sessions').textContent.includes('No sessions in this folder yet') && \
         byId('input').disabled",
    )
    .await;
    // All folders lists them with their folder; opening one switches back.
    page.eval(
        "byId('session-scope').value = 'all'; \
         byId('session-scope').dispatchEvent(new Event('change')); true",
    )
    .await;
    page.wait(
        "every folder's sessions",
        "sessionItems().length === 2 && \
         sessionItems().every((item) => item.textContent.includes('in workspace'))",
    )
    .await;
    page.eval("sessionItems()[0].click(); true").await;
    page.wait(
        "the session's own folder",
        "byId('status-root').textContent.endsWith('workspace') && \
         byId('notice').textContent.includes('Switched the workspace') && \
         !byId('input').disabled && entries().length > 0",
    )
    .await;
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
        "[...document.querySelectorAll('#settings-body button')].find((b) => b.textContent === '{label}').click()"
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
        "byId('settings-message').textContent === 'Model saved.'",
    )
    .await;
    page.send_message("/notool hi").await;
    page.wait(
        "the new label",
        "idle() && transcript().includes('[Z] you said: hi')",
    )
    .await;

    // A model that cannot start is reported, and the old one keeps working.
    page.eval(&format!(
        "{{ const kind = document.querySelector('#settings-body select'); kind.value = 'model-llamacpp'; \
         kind.dispatchEvent(new Event('change')); \
         document.querySelector('#settings-body input').value = 'http://example.com'; {}; true }}",
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

    // Tools: turn `echo` off; the default choice shows what applies.
    page.eval(&TAB.replace("{tab}", "tools")).await;
    page.wait(
        "the tools form",
        "!!document.querySelector('#settings-body select[data-tool=\"echo\"]')",
    )
    .await;
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
        save("Save tools")
    ))
    .await;
    page.wait(
        "the saved tools",
        "byId('settings-message').textContent === 'Tools saved.'",
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

    // Loop: switch to plan-and-execute.
    page.eval(&TAB.replace("{tab}", "loop")).await;
    page.wait(
        "the loop form",
        "!!document.querySelector('#settings-body select')",
    )
    .await;
    page.eval(&format!(
        "{{ const kind = document.querySelector('#settings-body select'); kind.value = 'loop-plan'; \
         kind.dispatchEvent(new Event('change')); {}; true }}",
        save("Save loop")
    ))
    .await;
    page.wait(
        "the new loop",
        "byId('settings-message').textContent === 'Loop saved.' && \
         byId('status-loop').textContent === 'loop: loop-plan'",
    )
    .await;
    page.eval("byId('settings-close').click(); true").await;
    assert_eq!(page.eval("byId('settings-dialog').open").await, false);

    page.eval("true").await;
    // The refused model answers 409; that network error is expected.
    let unexpected: Vec<_> = page
        .errors()
        .into_iter()
        .filter(|event| {
            !event["params"]["entry"]["text"]
                .as_str()
                .unwrap_or_default()
                .contains("409")
        })
        .collect();
    assert!(unexpected.is_empty(), "page errors: {unexpected:#?}");
}
