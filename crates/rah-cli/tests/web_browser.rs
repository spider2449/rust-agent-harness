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

/// How long a page condition may take.
const TIMEOUT: Duration = Duration::from_secs(15);
/// How long a DevTools command may take. The first page load after a build
/// took over 20 seconds on Windows (likely a scan of the new test binary).
const REPLY_TIMEOUT: Duration = Duration::from_secs(90);

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
                panic!("timed out waiting for {what}; page: {snapshot:#}");
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
}))()"#;

struct Web {
    _kernel: Kernel,
    url: String,
    _dir: TempDir,
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
        _dir: dir,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_web_ui_works_in_a_real_browser() {
    let Some(program) = std::env::var_os("RAH_TEST_BROWSER") else {
        eprintln!("skipped: set RAH_TEST_BROWSER to a Chromium-based browser to run this test");
        return;
    };
    let web = start("web-browser").await;
    let browser = Browser::launch(PathBuf::from(program), "web-browser");
    let mut page = browser.page().await;

    // Opening the printed URL: the token leaves the address bar and the
    // socket connects.
    page.call("Page.navigate", json!({ "url": web.url })).await;
    page.wait("the page to connect", "connected()").await;
    assert_eq!(page.eval("location.hash").await, "");
    assert_eq!(page.eval("byId('guard').textContent").await, "guarded");
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
}
