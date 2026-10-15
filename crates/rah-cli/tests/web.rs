//! P5 acceptance for `frontend-web`: access control, the session API, turns
//! over the WebSocket and `Ask` approvals answered by a browser.

mod common;

use std::sync::{Arc, Mutex};

use common::{TempDir, headless};
use futures::{SinkExt, StreamExt};
use rah_api::{Kernel, PluginId, host::FrontendListening};
use rah_cli::{Distribution, boot};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

struct Web {
    kernel: Kernel,
    address: String,
    token: String,
    dir: TempDir,
}

async fn start(name: &str, adjust: impl FnOnce(&mut Distribution)) -> Web {
    let dir = TempDir::new(name);
    let mut distribution = headless(&dir);
    distribution.plugin.push(toml_spec(
        r#"
[[plugin]]
use = "frontend-web"
config = { bind = "127.0.0.1:0" }
"#,
    ));
    adjust(&mut distribution);
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
    let rest = url.strip_prefix("http://").unwrap();
    let (address, token) = rest.split_once("/#token=").unwrap();
    Web {
        kernel,
        address: address.to_owned(),
        token: token.to_owned(),
        dir,
    }
}

fn toml_spec(text: &str) -> rah_cli::PluginSpec {
    Distribution::parse(text).unwrap().plugin.remove(0)
}

impl Web {
    fn get(&self, path: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .get(format!("http://{}{path}", self.address))
            .bearer_auth(&self.token)
    }

    fn post(&self, path: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .post(format!("http://{}{path}", self.address))
            .bearer_auth(&self.token)
    }

    async fn json(&self, request: reqwest::RequestBuilder) -> Value {
        let response = request.send().await.unwrap();
        assert!(response.status().is_success(), "{}", response.status());
        response.json().await.unwrap()
    }

    async fn socket(
        &self,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>
    {
        let url = format!("ws://{}/api/ws?token={}", self.address, self.token);
        tokio_tungstenite::connect_async(url).await.unwrap().0
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_json(socket: &mut Socket) -> Value {
    loop {
        match socket.next().await.unwrap().unwrap() {
            Message::Text(text) => return serde_json::from_str(&text).unwrap(),
            Message::Close(_) => panic!("socket closed"),
            _ => {}
        }
    }
}

async fn send(socket: &mut Socket, message: Value) {
    socket
        .send(Message::Text(message.to_string().into()))
        .await
        .unwrap();
}

/// Collects the model text of a turn until its terminal event; answers
/// approvals with `approve`.
async fn run_turn(
    socket: &mut Socket,
    session: &str,
    input: &str,
    approve: Option<bool>,
) -> (String, String) {
    send(
        socket,
        json!({ "type": "turn", "session": session, "input": input }),
    )
    .await;
    let mut text = String::new();
    loop {
        let message = next_json(socket).await;
        match message["type"].as_str().unwrap() {
            "approval_required" => {
                let approve = approve.expect("no approval expected");
                assert_eq!(message["tool"], "echo");
                send(
                    socket,
                    json!({ "type": "approval", "id": message["id"], "approve": approve }),
                )
                .await;
            }
            "approval_resolved" => {}
            "event" => {
                let event = &message["event"];
                match event["type"].as_str().unwrap() {
                    "model_delta" => text.push_str(event["delta"].as_str().unwrap()),
                    terminal @ ("completed" | "failed" | "cancelled") => {
                        return (terminal.to_owned(), text);
                    }
                    _ => {}
                }
            }
            other => panic!("unexpected {other}: {message}"),
        }
    }
}

#[tokio::test]
async fn the_api_requires_the_token_this_host_and_this_origin() {
    let web = start("web-access", |_| {}).await;
    let base = format!("http://{}", web.address);
    let client = reqwest::Client::new();

    let page = client.get(&base).send().await.unwrap();
    assert!(page.status().is_success());
    assert!(
        page.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("default-src 'self'")
    );
    assert!(page.text().await.unwrap().contains("<title>RAH</title>"));

    let status = |request: reqwest::RequestBuilder| async move {
        request.send().await.unwrap().status().as_u16()
    };
    assert_eq!(
        status(client.get(format!("{base}/api/sessions"))).await,
        401
    );
    assert_eq!(
        status(
            client
                .get(format!("{base}/api/sessions"))
                .bearer_auth("0".repeat(64))
        )
        .await,
        401
    );
    assert_eq!(status(web.get("/api/sessions")).await, 200);
    assert_eq!(
        status(web.get("/api/sessions").header("Host", "evil.example")).await,
        403
    );
    assert_eq!(
        status(client.get(&base).header("Host", "attacker.test:80")).await,
        403
    );
    assert_eq!(
        status(
            web.post("/api/sessions")
                .header("Origin", "http://evil.example")
        )
        .await,
        403
    );
    assert_eq!(
        status(
            web.post("/api/sessions")
                .header("Origin", format!("http://{}", web.address))
        )
        .await,
        200
    );

    let url = format!("ws://{}/api/ws", web.address)
        .into_client_request()
        .unwrap();
    assert!(tokio_tungstenite::connect_async(url).await.is_err());
}

#[tokio::test]
async fn sessions_turns_and_forks_work_over_http_and_the_socket() {
    let web = start("web-turns", |_| {}).await;
    let info = web.json(web.get("/api/info")).await;
    assert_eq!(info["guarded"], true);
    assert!(
        info["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "echo")
    );
    let plugins = web.json(web.get("/api/plugins")).await;
    assert!(
        plugins
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "frontend-web" && p["state"] == "active"),
        "{plugins}"
    );

    let session = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut socket = web.socket().await;
    let (end, text) = run_turn(&mut socket, &session, "hello", None).await;
    assert_eq!(
        (end.as_str(), text.as_str()),
        ("completed", "[model-mock] echo returned: hello")
    );

    let records = web.json(web.get(&format!("/api/sessions/{session}"))).await;
    assert_eq!(records.as_array().unwrap().len(), 5);
    assert_eq!(records[0]["entry"]["kind"], "turn_started");

    let fork = web
        .json(
            web.post(&format!("/api/sessions/{session}/fork"))
                .json(&json!({ "at": 1 })),
        )
        .await;
    assert_eq!(fork["parent"]["session"], session.as_str());
    let listed = web.json(web.get("/api/sessions")).await;
    assert_eq!(listed.as_array().unwrap().len(), 2);
    let missing = web.get("/api/sessions/not-a-session").send().await.unwrap();
    assert_eq!(missing.status().as_u16(), 400);

    send(
        &mut socket,
        json!({ "type": "turn", "session": "nope", "input": "x" }),
    )
    .await;
    assert_eq!(next_json(&mut socket).await["type"], "error");
}

#[tokio::test]
async fn ask_decisions_are_answered_by_the_browser() {
    let web = start("web-ask", |distribution| {
        distribution.set_config(
            "policy-authority",
            "tools",
            json!({ "echo": "write", "fs.read": "read", "repo.*": "read" }),
        );
    })
    .await;
    let session = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut socket = web.socket().await;
    let (_, text) = run_turn(&mut socket, &session, "approved call", Some(true)).await;
    assert_eq!(text, "[model-mock] echo returned: approved call");
    let (_, text) = run_turn(&mut socket, &session, "denied call", Some(false)).await;
    assert!(text.contains("denied: rejected by the user"), "{text}");
}

#[tokio::test]
async fn unmounting_the_frontend_closes_its_sockets() {
    let web = start("web-unmount", |_| {}).await;
    let mut socket = web.socket().await;
    web.kernel
        .unmount(&PluginId::new("frontend-web"))
        .await
        .unwrap();
    let ended = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(ended.is_ok(), "the socket stayed open after unmount");
}

impl Web {
    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        reqwest::Client::new()
            .request(method, format!("http://{}{path}", self.address))
            .bearer_auth(&self.token)
    }

    async fn status(&self, request: reqwest::RequestBuilder) -> (u16, Value) {
        let response = request.send().await.unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap_or(Value::Null))
    }
}

#[tokio::test]
async fn plugins_are_stopped_and_started_over_http() {
    use reqwest::Method;
    let web = start("web-stop", |_| {}).await;
    let state = |plugins: &Value, id: &str| {
        plugins
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .map(|p| (p["state"].clone(), p["kind"].clone()))
    };
    let session = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut socket = web.socket().await;

    let (status, reply) = web.status(web.post("/api/plugins/model-mock/stop")).await;
    assert_eq!(
        (status, reply["state"].as_str()),
        (200, Some("stopped")),
        "{reply}"
    );
    let plugins = web.json(web.get("/api/plugins")).await;
    assert_eq!(
        state(&plugins, "model-mock"),
        Some((json!("stopped"), json!("model-mock"))),
        "{plugins}"
    );
    let (terminal, _) = run_turn(&mut socket, &session, "one", None).await;
    assert_eq!(terminal, "failed");

    // While stopped: the id stays taken, and only start and remove apply.
    for (method, path, expected) in [
        (Method::POST, "/api/plugins/model-mock/stop", 409),
        (Method::POST, "/api/plugins/model-mock/restart", 409),
        (Method::PUT, "/api/plugins/model-mock/config", 409),
    ] {
        let request = web.request(method, path).json(&json!({ "config": {} }));
        assert_eq!(web.status(request).await.0, expected, "{path}");
    }
    let (status, _) = web
        .status(
            web.post("/api/plugins")
                .json(&json!({ "kind": "model-mock" })),
        )
        .await;
    assert_eq!(status, 409);

    let (status, reply) = web.status(web.post("/api/plugins/model-mock/start")).await;
    assert_eq!(
        (status, reply["state"].as_str()),
        (200, Some("active")),
        "{reply}"
    );
    let (_, text) = run_turn(&mut socket, &session, "two", None).await;
    assert_eq!(text, "[model-mock] echo returned: two");
    assert_eq!(
        web.status(web.post("/api/plugins/model-mock/start"))
            .await
            .0,
        409
    );

    // Policy plugins and the distribution cannot be stopped.
    assert_eq!(
        web.status(web.post("/api/plugins/policy-authority/stop"))
            .await
            .0,
        400
    );
    assert_eq!(
        web.status(web.post("/api/plugins/distribution/stop"))
            .await
            .0,
        403
    );
    assert_eq!(
        web.status(web.post("/api/plugins/missing/stop")).await.0,
        404
    );
    assert_eq!(
        web.status(web.post("/api/plugins/missing/start")).await.0,
        404
    );

    // Removing a stopped plugin forgets it.
    web.status(web.post("/api/plugins/model-mock/stop")).await;
    assert_eq!(
        web.status(web.request(Method::DELETE, "/api/plugins/model-mock"))
            .await
            .0,
        200
    );
    let plugins = web.json(web.get("/api/plugins")).await;
    assert_eq!(state(&plugins, "model-mock"), None);
}

#[tokio::test]
async fn plugins_are_managed_and_the_model_and_loop_switch_without_a_restart() {
    use reqwest::Method;
    let web = start("web-manage", |_| {}).await;
    let catalog = web.json(web.get("/api/catalog")).await;
    let kinds = catalog["kinds"].as_array().unwrap();
    assert!(
        kinds.contains(&json!("model-mock")) && kinds.contains(&json!("process")),
        "{catalog}"
    );
    // Every kind comes with an example configuration for the editor.
    for kind in kinds {
        let example = &catalog["examples"][kind.as_str().unwrap()];
        assert!(example.is_object(), "{kind}: {example}");
    }
    assert_eq!(
        catalog["examples"]["loop-react"],
        json!({ "max_rounds": 32 })
    );
    // Kinds with help describe their keys for the editor.
    let help_keys = |kind: &str| -> Vec<String> {
        catalog["help"][kind]
            .as_array()
            .unwrap_or_else(|| panic!("{kind}: {catalog}"))
            .iter()
            .map(|entry| entry["key"].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(help_keys("sandbox-process").contains(&"inherit_env_extra".to_owned()));
    assert!(help_keys("sandbox-os").contains(&"inherit_env_extra".to_owned()));
    assert!(help_keys("tools-shell").contains(&"allow_programs".to_owned()));
    assert!(catalog["help"]["loop-react"].is_null(), "{catalog}");
    let plugins = web.json(web.get("/api/plugins")).await;
    let find = |plugins: &Value, id: &str| {
        plugins
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .cloned()
    };
    assert_eq!(find(&plugins, "model-mock").unwrap()["kind"], "model-mock");
    assert_eq!(find(&plugins, "distribution").unwrap()["locked"], true);

    // One socket and one session for the whole test: nothing restarts.
    let session = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut socket = web.socket().await;
    let (_, text) = run_turn(&mut socket, &session, "one", None).await;
    assert_eq!(text, "[model-mock] echo returned: one");

    let (status, reply) = web
        .status(
            web.post("/api/plugins/model-mock/replace")
                .json(&json!({ "kind": "model-mock", "config": { "label": "B" } })),
        )
        .await;
    assert_eq!(
        (status, reply["id"].as_str(), reply["state"].as_str()),
        (200, Some("model-mock-2"), Some("active")),
        "{reply}"
    );
    let (_, text) = run_turn(&mut socket, &session, "two", None).await;
    assert_eq!(text, "[B] echo returned: two");

    let (status, reply) = web
        .status(
            web.post("/api/plugins/loop-react/replace").json(
                &json!({ "kind": "loop-react", "id": "loop-b", "config": { "max_rounds": 4 } }),
            ),
        )
        .await;
    assert_eq!(
        (status, reply["id"].as_str()),
        (200, Some("loop-b")),
        "{reply}"
    );
    let (_, text) = run_turn(&mut socket, &session, "/notool three", None).await;
    // Two earlier turns of four context items each: the history survived.
    assert_eq!(text, "[B] you said: three (8 earlier items)");

    let (status, _) = web
        .status(
            web.request(Method::PUT, "/api/plugins/model-mock-2/config")
                .json(&json!({ "config": { "label": "C" } })),
        )
        .await;
    assert_eq!(status, 200);
    let (_, text) = run_turn(&mut socket, &session, "four", None).await;
    assert_eq!(text, "[C] echo returned: four");
    assert_eq!(
        web.status(web.post("/api/plugins/loop-b/restart")).await.0,
        200
    );

    // A replacement with a refused configuration, or one that does not
    // start, keeps the plugin it would replace.
    let (status, reply) = web
        .status(web.post("/api/plugins/model-mock-2/replace").json(
            &json!({ "kind": "model-llamacpp", "config": { "endpoint": "http://example.com" } }),
        ))
        .await;
    assert_eq!(status, 400, "{reply}");
    assert!(
        reply["error"]
            .as_str()
            .unwrap()
            .contains("invalid configuration"),
        "{reply}"
    );
    let (status, reply) = web
        .status(web.post("/api/plugins/model-mock-2/replace").json(
            &json!({ "kind": "model-llamacpp", "config": { "api_key_env": "RAH_TEST_UNSET_KEY_51E0" } }),
        ))
        .await;
    assert_eq!(status, 409, "{reply}");
    let (_, text) = run_turn(&mut socket, &session, "five", None).await;
    assert_eq!(text, "[C] echo returned: five");

    let (status, _) = web
        .status(
            web.post("/api/plugins")
                .json(&json!({ "kind": "model-mock", "id": "spare", "config": { "label": "S" } })),
        )
        .await;
    assert_eq!(status, 200);
    assert_eq!(
        web.status(
            web.post("/api/plugins")
                .json(&json!({ "kind": "model-mock", "id": "spare" }))
        )
        .await
        .0,
        409
    );
    assert_eq!(
        web.status(web.request(Method::DELETE, "/api/plugins/spare"))
            .await
            .0,
        200
    );
    let plugins = web.json(web.get("/api/plugins")).await;
    assert!(find(&plugins, "spare").is_none());
    assert!(find(&plugins, "model-mock").is_none() && find(&plugins, "loop-react").is_none());

    assert_eq!(
        web.status(web.request(Method::DELETE, "/api/plugins/distribution"))
            .await
            .0,
        403
    );
    assert_eq!(
        web.status(web.request(Method::DELETE, "/api/plugins/missing"))
            .await
            .0,
        404
    );
    assert_eq!(
        web.status(
            web.post("/api/plugins")
                .json(&json!({ "kind": "model-gpt9" }))
        )
        .await
        .0,
        400
    );
    let unauthorized = reqwest::Client::new()
        .post(format!("http://{}/api/plugins", web.address))
        .json(&json!({ "kind": "model-mock", "id": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status().as_u16(), 401);
}

#[tokio::test]
async fn background_jobs_are_submitted_listed_and_cancelled() {
    use reqwest::Method;
    let web = start("web-jobs", |_| {}).await;
    let job = web
        .json(
            web.post("/api/jobs")
                .json(&json!({ "input": "/notool later", "delay_secs": 3600 })),
        )
        .await;
    assert_eq!(job["state"], "waiting", "{job}");
    let id = job["id"].as_str().unwrap().to_owned();
    let jobs = web.json(web.get("/api/jobs")).await;
    assert!(
        jobs.as_array()
            .unwrap()
            .iter()
            .any(|j| j["id"] == id.as_str())
    );
    assert_eq!(
        web.status(web.request(Method::DELETE, &format!("/api/jobs/{id}")))
            .await
            .0,
        200
    );
    assert_eq!(
        web.status(web.request(Method::DELETE, &format!("/api/jobs/{id}")))
            .await
            .0,
        404
    );
    assert_eq!(
        web.status(web.post("/api/jobs").json(&json!({ "input": "" })))
            .await
            .0,
        400
    );
}

#[tokio::test]
async fn sessions_are_titled_renamed_exported_and_deleted_over_http() {
    use reqwest::Method;
    let web = start("web-session-ops", |_| {}).await;
    let session = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut socket = web.socket().await;
    run_turn(&mut socket, &session, "hello", None).await;

    // `session-titles` names the session after its first turn.
    let find = |list: &Value| {
        list.as_array()
            .unwrap()
            .iter()
            .find(|info| info["id"] == session.as_str())
            .cloned()
            .unwrap()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let titled = loop {
        let info = find(&web.json(web.get("/api/sessions")).await);
        if info["title"].is_string() {
            break info;
        }
        assert!(std::time::Instant::now() < deadline, "no title: {info}");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    assert_eq!(
        titled["title"], "[model-mock] you said: User: hello",
        "{titled}"
    );
    assert_eq!(titled["preview"], "hello");
    assert!(titled["updated"].as_u64().unwrap() >= titled["created"].as_u64().unwrap());

    let path = format!("/api/sessions/{session}/title");
    let (status, renamed) = web
        .status(
            web.request(Method::PUT, &path)
                .json(&json!({ "title": "  Kyoto\n trip " })),
        )
        .await;
    assert_eq!(
        (status, renamed["title"].as_str()),
        (200, Some("Kyoto trip"))
    );
    assert_eq!(
        web.status(
            web.request(Method::PUT, &path)
                .json(&json!({ "title": " " }))
        )
        .await
        .0,
        400
    );

    let export = web
        .get(&format!("/api/sessions/{session}/export?format=md"))
        .send()
        .await
        .unwrap();
    assert_eq!(export.status().as_u16(), 200);
    assert_eq!(
        export.headers()["content-disposition"],
        format!("attachment; filename=\"rah-session-{session}.md\"").as_str()
    );
    assert!(export.text().await.unwrap().starts_with("# Kyoto trip\n"));
    let json_export = web
        .json(web.get(&format!("/api/sessions/{session}/export?format=json")))
        .await;
    assert_eq!(json_export["session"]["title"], "Kyoto trip");
    assert_eq!(json_export["records"].as_array().unwrap().len(), 5);
    assert_eq!(
        web.status(web.get(&format!("/api/sessions/{session}/export?format=pdf")))
            .await
            .0,
        400
    );

    // A parent with forks is kept; deleting the fork first allows it.
    let fork = web
        .json(
            web.post(&format!("/api/sessions/{session}/fork"))
                .json(&json!({ "at": 1 })),
        )
        .await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, refused) = web
        .status(web.request(Method::DELETE, &format!("/api/sessions/{session}")))
        .await;
    assert_eq!(status, 409, "{refused}");
    assert!(refused["error"].as_str().unwrap().contains(&fork));
    for id in [&fork, &session] {
        assert_eq!(
            web.status(web.request(Method::DELETE, &format!("/api/sessions/{id}")))
                .await
                .0,
            200
        );
    }
    assert_eq!(web.json(web.get("/api/sessions")).await, json!([]));
    assert_eq!(
        web.status(web.request(Method::DELETE, &format!("/api/sessions/{session}")))
            .await
            .0,
        404
    );
}

#[tokio::test]
async fn the_status_line_reports_the_workspace_git_state_and_providers() {
    let web = start("web-status", |_| {}).await;
    let status = web.json(web.get("/api/status")).await;
    assert_eq!(status["model"]["plugin"], "model-mock", "{status}");
    assert_eq!(status["model"]["kind"], "model-mock");
    assert_eq!(status["loop"]["plugin"], "loop-react");
    assert_eq!(status["guarded"], true);
    assert!(status["tools"].as_u64().unwrap() > 0);
    let root = status["workspace"]["root"].as_str().unwrap().to_owned();
    assert!(
        !root.starts_with(r"\?\") && root.ends_with("workspace"),
        "{root}"
    );
    // The temporary workspace is not a Git repository yet.
    assert!(status["workspace"]["git"]["error"].is_string(), "{status}");

    let workspace = std::path::PathBuf::from(&root);
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&workspace)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
    };
    git(&["init", "-q", "-b", "trunk"]);
    std::fs::write(workspace.join("notes.txt"), "draft").unwrap();
    let git_state = web.json(web.get("/api/status")).await["workspace"]["git"].clone();
    assert_eq!(git_state["branch"], "trunk", "{git_state}");
    assert_eq!(git_state["untracked"], 1, "{git_state}");
    assert_eq!(git_state["changed"], 0);
    assert_eq!(git_state["upstream"], Value::Null);
}

#[tokio::test]
async fn the_workspace_is_browsed_and_switched_without_a_restart() {
    use reqwest::Method;
    let web = start("web-workspace", |_| {}).await;
    let other = web.dir.0.join("other");
    std::fs::create_dir_all(other.join(".git")).unwrap();
    std::fs::write(other.join("note.txt"), "from the other folder").unwrap();

    // The folder browser lists subdirectories and marks Git repositories.
    let base = web.dir.0.display().to_string();
    let listing = web
        .json(web.get(&format!(
            "/api/fs/dirs?path={}",
            base.replace('\\', "%5C").replace(':', "%3A")
        )))
        .await;
    let dirs = listing["dirs"].as_array().unwrap();
    let entry = |name: &str| dirs.iter().find(|dir| dir["name"] == name).cloned();
    assert_eq!(entry("other").unwrap()["git"], true, "{listing}");
    assert_eq!(entry("workspace").unwrap()["git"], false);
    assert!(listing["parent"].is_string());
    assert!(!listing["roots"].as_array().unwrap().is_empty());
    // Without a path it starts at the current workspace.
    let current = web.json(web.get("/api/fs/dirs")).await;
    assert!(
        current["path"].as_str().unwrap().ends_with("workspace"),
        "{current}"
    );

    let session = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut socket = web.socket().await;
    let (status, switched) = web
        .status(
            web.request(Method::PUT, "/api/workspace")
                .json(&json!({ "root": other.display().to_string() })),
        )
        .await;
    assert_eq!(status, 200, "{switched}");
    assert!(
        !switched["root"].as_str().unwrap().starts_with(r"\?\"),
        "{switched}"
    );
    let changed: Vec<&str> = switched["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|plugin| {
            assert_eq!(plugin["state"], "active", "{switched}");
            plugin["id"].as_str().unwrap()
        })
        .collect();
    assert_eq!(
        changed,
        ["sandbox-process", "tools-builtin", "tools-repo-write"]
    );

    // The same socket and session; tools now work in the other folder.
    let (_, text) = run_turn(&mut socket, &session, "read note.txt", None).await;
    assert_eq!(text, "[model-mock] fs.read returned: from the other folder");

    // The memory notes are the new folder's own: a new session there sees
    // them (one system item before the message).
    let root = switched["root"].as_str().unwrap();
    let memory = web.dir.0.join("memory");
    std::fs::create_dir_all(&memory).unwrap();
    std::fs::write(
        memory.join(rah_plugin_context_default::workspace_file_name(
            std::path::Path::new(root),
        )),
        "- the other folder uses tabs\n",
    )
    .unwrap();
    let fresh = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, text) = run_turn(&mut socket, &fresh, "/notool hi", None).await;
    assert_eq!(text, "[model-mock] you said: hi (1 earlier items)");

    let status_line = web.json(web.get("/api/status")).await;
    assert!(
        status_line["workspace"]["root"]
            .as_str()
            .unwrap()
            .ends_with("other"),
        "{status_line}"
    );

    for bad in [
        json!({ "root": web.dir.0.join("missing").display().to_string() }),
        json!({ "root": other.join("note.txt").display().to_string() }),
        json!({ "root": " " }),
    ] {
        assert_eq!(
            web.status(web.request(Method::PUT, "/api/workspace").json(&bad))
                .await
                .0,
            400,
            "{bad}"
        );
    }
}

#[tokio::test]
async fn workspace_files_are_listed_and_read_inside_the_workspace_only() {
    let web = start("web-files", |_| {}).await;
    let workspace = web.dir.workspace();
    std::fs::create_dir_all(workspace.join("src").join("ui")).unwrap();
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    std::fs::write(workspace.join("src").join("main.rs"), "fn main() {}\n").unwrap();
    std::fs::write(workspace.join("README.md"), "# Notes\n").unwrap();
    std::fs::write(workspace.join("blob.bin"), [0_u8, 1, 2, 3]).unwrap();
    std::fs::write(web.dir.0.join("secret.txt"), "outside").unwrap();

    // Folders first, then files; Git's own data is not listed.
    let root = web.json(web.get("/api/files")).await;
    let names: Vec<&str> = root["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["src", "blob.bin", "README.md"], "{root}");
    assert_eq!(root["entries"][0]["dir"], true);
    assert_eq!(root["path"], "");
    assert!(
        root["root"].as_str().unwrap().ends_with("workspace"),
        "{root}"
    );
    let src = web.json(web.get("/api/files?path=src")).await;
    assert_eq!(src["entries"][0]["path"], "src/ui", "{src}");
    assert_eq!(src["entries"][1]["path"], "src/main.rs");

    let file = web
        .json(web.get("/api/files/content?path=src%2Fmain.rs"))
        .await;
    assert_eq!(file["text"], "fn main() {}\n", "{file}");
    assert_eq!(file["binary"], false);
    assert_eq!(file["truncated"], false);
    let blob = web.json(web.get("/api/files/content?path=blob.bin")).await;
    assert_eq!(blob["binary"], true, "{blob}");
    assert_eq!(blob["text"], "");

    // Nothing outside the workspace, however it is named.
    let absolute = web.dir.0.join("secret.txt").display().to_string();
    for path in [
        "..%2Fsecret.txt",
        "src%2F..%2F..%2Fsecret.txt",
        &absolute
            .replace('\\', "%5C")
            .replace(':', "%3A")
            .replace('/', "%2F"),
    ] {
        let (status, body) = web
            .status(web.get(&format!("/api/files/content?path={path}")))
            .await;
        assert_eq!(status, 400, "{path}: {body}");
        assert!(body["text"].is_null(), "{body}");
    }
    let (status, _) = web.status(web.get("/api/files/content?path=src")).await;
    assert_eq!(status, 400, "a folder is not a file");
    let (status, _) = web.status(web.get("/api/files?path=missing")).await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn git_changes_and_diffs_are_shown_read_only() {
    let web = start("web-git", |_| {}).await;
    let workspace = web.dir.workspace();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
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
    // Not a repository yet: the view says so.
    let changes = web.json(web.get("/api/git/changes")).await;
    assert!(changes["error"].is_string(), "{changes}");

    git(&["init", "-q", "-b", "trunk"]);
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::write(workspace.join("src").join("lib.rs"), "one\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "first commit"]);
    std::fs::write(workspace.join("src").join("lib.rs"), "one\ntwo\n").unwrap();
    std::fs::write(workspace.join("notes.txt"), "draft").unwrap();

    let changes = web.json(web.get("/api/git/changes")).await;
    assert_eq!(changes["branch"], "trunk", "{changes}");
    let brief: Vec<(String, String)> = changes["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| {
            (
                change["path"].as_str().unwrap().to_owned(),
                change["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert!(
        brief.contains(&("src/lib.rs".into(), "M".into())),
        "{changes}"
    );
    assert!(
        brief.contains(&("notes.txt".into(), "U".into())),
        "{changes}"
    );

    let diff = web.json(web.get("/api/git/diff?path=src%2Flib.rs")).await;
    let text = diff["text"].as_str().unwrap();
    assert!(text.contains("\n+two"), "{diff}");
    assert_eq!(diff["truncated"], false);

    // Only plain paths inside the workspace.
    for refused in [
        "/api/git/diff?path=..%2Fsecret",
        "/api/git/diff?path=%2Fetc%2Fpasswd",
        "/api/git/diff?path=",
    ] {
        let (status, body) = web.status(web.get(refused)).await;
        assert_eq!(status, 400, "{refused}: {body}");
    }
}

/// Waits until `kind`'s default instance is active in `kernel`.
async fn wait_active(kernel: &Kernel, id: &str) -> bool {
    for _ in 0..200 {
        let active = kernel.plugins().into_iter().any(|(plugin, status)| {
            plugin.as_str() == id && status == rah_api::PluginStatus::Active
        });
        if active {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    false
}

#[tokio::test]
async fn web_search_is_switched_on_asks_first_and_is_remembered() {
    use reqwest::Method;
    let web = start("web-search-switch", |_| {}).await;
    let state = web.json(web.get("/api/web-search")).await;
    assert_eq!(state, json!({ "available": true, "enabled": false }));

    let (status, on) = web
        .status(
            web.request(Method::PUT, "/api/web-search")
                .json(&json!({ "enabled": true })),
        )
        .await;
    assert_eq!(status, 200, "{on}");
    assert_eq!(on["enabled"], true, "{on}");
    assert_eq!(on["remembered"], true, "{on}");
    let plugins = web.json(web.get("/api/plugins")).await;
    let policy = plugins
        .as_array()
        .unwrap()
        .iter()
        .find(|plugin| plugin["kind"] == "policy-authority")
        .unwrap();
    assert_eq!(policy["config"]["decisions"]["web.*"], "ask", "{policy}");
    // The preset's own rules are kept.
    assert_eq!(policy["config"]["tools"]["fs.*"], "read", "{policy}");
    let info = web.json(web.get("/api/info")).await;
    assert!(
        info["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "web.search"),
        "{info}"
    );

    // A call asks first; refusing it sends nothing anywhere.
    let session = web.json(web.post("/api/sessions")).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut socket = web.socket().await;
    let input = r#"/call web.search {"query":"rah"}"#;
    send(
        &mut socket,
        json!({ "type": "turn", "session": session, "input": input }),
    )
    .await;
    let mut asked = false;
    let mut text = String::new();
    loop {
        let message = next_json(&mut socket).await;
        match message["type"].as_str().unwrap() {
            "approval_required" => {
                assert_eq!(message["tool"], "web.search", "{message}");
                asked = true;
                send(
                    &mut socket,
                    json!({ "type": "approval", "id": message["id"], "approve": false }),
                )
                .await;
            }
            "event" => match message["event"]["type"].as_str().unwrap() {
                "model_delta" => text.push_str(message["event"]["delta"].as_str().unwrap()),
                "completed" | "failed" | "cancelled" => break,
                _ => {}
            },
            _ => {}
        }
    }
    assert!(asked);
    assert!(text.contains("denied: rejected by the user"), "{text}");

    // A new start with the same storage turns it on again.
    let mut distribution = headless(&web.dir);
    distribution.plugin.push(toml_spec(
        r#"
[[plugin]]
use = "frontend-web"
config = { bind = "127.0.0.1:0" }
"#,
    ));
    let restarted = Kernel::new();
    boot(&restarted, &distribution).await.unwrap();
    assert!(wait_active(&restarted, "tools-web-search").await);

    let (status, off) = web
        .status(
            web.request(Method::PUT, "/api/web-search")
                .json(&json!({ "enabled": false })),
        )
        .await;
    assert_eq!(status, 200, "{off}");
    assert_eq!(off["enabled"], false, "{off}");
    assert!(
        !web.kernel
            .plugins()
            .iter()
            .any(|(id, _)| id.as_str() == "tools-web-search")
    );
    // Off is remembered too.
    let again = Kernel::new();
    boot(&again, &distribution).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        !again
            .plugins()
            .iter()
            .any(|(id, _)| id.as_str() == "tools-web-search")
    );
}

/// The kind and configuration of `kernel`'s plugin `id`, through `admin`.
fn instance(kernel: &Kernel, id: &str) -> Option<(String, Value)> {
    let admin = kernel.handle().get(rah_api::admin::ADMIN)?;
    let instance = admin.instance(&PluginId::new(id))?;
    Some((instance.kind, instance.config))
}

/// Waits until `kernel` has an active plugin of `kind`; returns its id.
async fn wait_kind(kernel: &Kernel, kind: &str) -> Option<String> {
    for _ in 0..200 {
        let found = kernel.plugins().into_iter().find(|(id, status)| {
            *status == rah_api::PluginStatus::Active
                && instance(kernel, id.as_str()).is_some_and(|(k, _)| k == kind)
        });
        if let Some((id, _)) = found {
            return Some(id.as_str().to_owned());
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    None
}

#[tokio::test]
async fn settings_are_saved_rolled_back_when_invalid_and_restored_at_start() {
    use reqwest::Method;
    let web = start("web-settings-saved", |_| {}).await;
    let put = |slot: &str, body: Value| {
        web.request(Method::PUT, &format!("/api/settings/{slot}"))
            .json(&body)
    };

    // The model keeps its kind: configured in place.
    let (status, saved) = web
        .status(put(
            "model",
            json!({ "id": "model-mock", "kind": "model-mock", "config": { "label": "Z" } }),
        ))
        .await;
    assert_eq!(status, 200, "{saved}");
    assert_eq!(saved["state"], "active", "{saved}");
    assert_eq!(saved["remembered"], true, "{saved}");
    // The loop changes kind: replaced.
    let (status, saved) = web
        .status(put(
            "loop",
            json!({ "id": "loop-react", "kind": "loop-plan", "config": { "max_steps": 3 } }),
        ))
        .await;
    assert_eq!(status, 200, "{saved}");
    // The policy: one decision changed, the rest kept.
    let (_, policy) = instance(&web.kernel, "policy-authority").unwrap();
    let mut changed = policy.clone();
    changed["decisions"] = json!({ "echo": "deny" });
    let (status, saved) = web
        .status(put(
            "tools",
            json!({ "id": "policy-authority", "kind": "policy-authority", "config": changed }),
        ))
        .await;
    assert_eq!(status, 200, "{saved}");

    // A refused policy configuration is not applied and not remembered;
    // tool calls stay guarded.
    let (status, refused) = web
        .status(put(
            "tools",
            json!({ "id": "policy-authority", "kind": "policy-authority", "config": { "unknown": 1 } }),
        ))
        .await;
    assert_eq!(status, 400, "{refused}");
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("invalid configuration for plugin `policy-authority`"),
        "{refused}"
    );
    assert!(wait_active(&web.kernel, "policy-authority").await);
    assert_eq!(
        instance(&web.kernel, "policy-authority").unwrap().1,
        changed
    );
    // Unknown slots and kinds that do not fit a slot are refused.
    let (status, _) = web
        .status(put("nothing", json!({ "kind": "model-mock" })))
        .await;
    assert_eq!(status, 400);
    let (status, _) = web
        .status(put("model", json!({ "kind": "loop-plan" })))
        .await;
    assert_eq!(status, 400);

    // A new start with the same storage applies all three.
    let mut distribution = headless(&web.dir);
    distribution.plugin.push(toml_spec(
        r#"
[[plugin]]
use = "frontend-web"
config = { bind = "127.0.0.1:0" }
"#,
    ));
    let restarted = Kernel::new();
    boot(&restarted, &distribution).await.unwrap();
    let plan = wait_kind(&restarted, "loop-plan").await.expect("loop-plan");
    assert_eq!(
        instance(&restarted, &plan).unwrap().1,
        json!({ "max_steps": 3 })
    );
    assert!(wait_kind(&restarted, "loop-react").await.is_none());
    for _ in 0..200 {
        let model = instance(&restarted, "model-mock").map(|(_, config)| config);
        let policy = instance(&restarted, "policy-authority").map(|(_, config)| config);
        if model == Some(json!({ "label": "Z" })) && policy.as_ref() == Some(&changed) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(
        instance(&restarted, "model-mock").unwrap().1,
        json!({ "label": "Z" })
    );
    assert_eq!(instance(&restarted, "policy-authority").unwrap().1, changed);
}

#[tokio::test]
async fn preferences_are_kept_bounded_and_survive_a_restart() {
    use reqwest::Method;
    let web = start("web-preferences", |_| {}).await;
    let put = |body: Value| web.request(Method::PUT, "/api/preferences").json(&body);
    let recent: Vec<String> = (0..10).map(|n| format!("C:/work/{n}")).collect();
    let (status, saved) = web
        .status(put(json!({
            "raw_markdown": true,
            "folders": { "c:/work/0": false, "": true },
            "recent_workspaces": recent,
            "side": { "open": true, "view": "plugins", "width": 480, "tree_ratio": 0.4 },
        })))
        .await;
    assert_eq!(status, 200, "{saved}");
    assert_eq!(saved["remembered"], true, "{saved}");
    // Only the latest recent workspaces are kept.
    assert_eq!(saved["recent_workspaces"].as_array().unwrap().len(), 8);

    // Refused: unknown fields, an unknown view, too many folders.
    for body in [
        json!({ "colour": "red" }),
        json!({ "side": { "view": "elsewhere" } }),
        json!({ "folders": (0..1001).map(|n| (n.to_string(), json!(true))).collect::<serde_json::Map<_, _>>() }),
    ] {
        let (status, refused) = web.status(put(body)).await;
        // 422 from the JSON extractor, 400 from the checks.
        assert!(status == 400 || status == 422, "{status}: {refused}");
    }

    // A new start with the same storage reads them back.
    let mut distribution = headless(&web.dir);
    distribution.plugin.push(toml_spec(
        r#"
[[plugin]]
use = "frontend-web"
config = { bind = "127.0.0.1:0" }
"#,
    ));
    web.kernel.shutdown().await.unwrap();
    let restarted = Kernel::new();
    let url = Arc::new(Mutex::new(None));
    let seen = url.clone();
    let _subscription = restarted
        .handle()
        .subscribe(move |listening: &FrontendListening| {
            *seen.lock().unwrap() = Some(listening.url.clone());
        });
    boot(&restarted, &distribution).await.unwrap();
    let url = url.lock().unwrap().clone().unwrap();
    let (address, token) = url
        .strip_prefix("http://")
        .unwrap()
        .split_once("/#token=")
        .unwrap();
    let stored: Value = reqwest::Client::new()
        .get(format!("http://{address}/api/preferences"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stored["raw_markdown"], true, "{stored}");
    assert_eq!(stored["folders"], json!({ "c:/work/0": false, "": true }));
    assert_eq!(stored["recent_workspaces"][0], "C:/work/0");
    assert_eq!(stored["side"]["view"], "plugins");
    restarted.shutdown().await.unwrap();
}

#[tokio::test]
async fn plugins_panel_changes_are_applied_again_at_the_next_start() {
    use reqwest::Method;
    let web = start("web-panel-kept", |_| {}).await;
    let ok = |(status, body): (u16, Value)| assert_eq!(status, 200, "{body}");

    // Mount a second model, configure one from the distribution, stop one
    // and remove one.
    ok(web
        .status(web.request(Method::POST, "/api/plugins").json(
            &json!({ "kind": "model-mock", "id": "model-extra", "config": { "label": "X" } }),
        ))
        .await);
    ok(web
        .status(
            web.request(Method::PUT, "/api/plugins/model-mock/config")
                .json(&json!({ "config": { "label": "Y" } })),
        )
        .await);
    ok(web
        .status(web.request(Method::POST, "/api/plugins/scheduler-basic/stop"))
        .await);
    ok(web
        .status(web.request(Method::DELETE, "/api/plugins/session-titles"))
        .await);
    // Removing the policy works now, but is not carried to the next start.
    ok(web
        .status(web.request(Method::DELETE, "/api/plugins/policy-authority"))
        .await);
    web.kernel.shutdown().await.unwrap();

    let mut distribution = headless(&web.dir);
    distribution.plugin.push(toml_spec(
        r#"
[[plugin]]
use = "frontend-web"
config = { bind = "127.0.0.1:0" }
"#,
    ));
    let restarted = Kernel::new();
    boot(&restarted, &distribution).await.unwrap();
    let admin = restarted.handle().get(rah_api::admin::ADMIN).unwrap();
    let mut applied = false;
    for _ in 0..200 {
        applied = instance(&restarted, "model-extra").map(|(_, c)| c)
            == Some(json!({ "label": "X" }))
            && instance(&restarted, "model-mock").map(|(_, c)| c) == Some(json!({ "label": "Y" }))
            && admin.stopped() == [PluginId::new("scheduler-basic")]
            && instance(&restarted, "session-titles").is_none();
        if applied {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(applied, "plugins: {:?}", restarted.plugins());
    assert!(wait_active(&restarted, "policy-authority").await);
    restarted.shutdown().await.unwrap();
}

#[tokio::test]
async fn several_sessions_are_deleted_at_once_forks_first() {
    let web = start("web-delete-many", |_| {}).await;
    let web = &web;
    let new_session = || async {
        web.json(web.post("/api/sessions")).await["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let fork = |parent: String| async move {
        web.json(
            web.post(&format!("/api/sessions/{parent}/fork"))
                .json(&json!({ "at": 0 })),
        )
        .await["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    // root <- child <- grandchild, listed parents first; and a lone session.
    let root = new_session().await;
    let child = fork(root.clone()).await;
    let grandchild = fork(child.clone()).await;
    let lone = new_session().await;
    let kept = new_session().await;

    let reply = web
        .json(
            web.post("/api/sessions/delete")
                .json(&json!({ "ids": [root, child, grandchild, lone] })),
        )
        .await;
    assert_eq!(reply["failed"], json!([]), "{reply}");
    let mut deleted: Vec<&str> = reply["deleted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap())
        .collect();
    // Forks went before the sessions they came from.
    let position = |id: &str| deleted.iter().position(|d| *d == id).unwrap();
    assert!(position(&grandchild) < position(&child));
    assert!(position(&child) < position(&root));
    deleted.sort_unstable();
    let mut expected = vec![
        root.as_str(),
        child.as_str(),
        grandchild.as_str(),
        lone.as_str(),
    ];
    expected.sort_unstable();
    assert_eq!(deleted, expected);
    let listed = web.json(web.get("/api/sessions")).await;
    let ids: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|info| info["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [kept.as_str()]);

    // A parent whose fork is not in the request is refused; the rest goes.
    let parent = new_session().await;
    let outside = fork(parent.clone()).await;
    let other = new_session().await;
    let reply = web
        .json(
            web.post("/api/sessions/delete")
                .json(&json!({ "ids": [parent, other, "00000000-0000-0000-0000-000000000000"] })),
        )
        .await;
    assert_eq!(reply["deleted"], json!([other]), "{reply}");
    let failed = reply["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 2, "{reply}");
    let refusal = failed.iter().find(|f| f["id"] == parent.as_str()).unwrap();
    assert!(
        refusal["error"].as_str().unwrap().contains(&outside),
        "{reply}"
    );

    // Not a session id: nothing is deleted.
    let (status, _) = web
        .status(
            web.post("/api/sessions/delete")
                .json(&json!({ "ids": [kept, "nope"] })),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(
        web.json(web.get("/api/sessions"))
            .await
            .as_array()
            .unwrap()
            .len(),
        3
    );
}
