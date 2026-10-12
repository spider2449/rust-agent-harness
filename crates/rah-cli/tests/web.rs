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
    _dir: TempDir,
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
        _dir: dir,
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
async fn plugins_are_managed_and_the_model_and_loop_switch_without_a_restart() {
    use reqwest::Method;
    let web = start("web-manage", |_| {}).await;
    let catalog = web.json(web.get("/api/catalog")).await;
    let kinds = catalog["kinds"].as_array().unwrap();
    assert!(
        kinds.contains(&json!("model-mock")) && kinds.contains(&json!("process")),
        "{catalog}"
    );
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

    // A replacement that does not start keeps the plugin it would replace.
    let (status, reply) = web
        .status(web.post("/api/plugins/model-mock-2/replace").json(
            &json!({ "kind": "model-llamacpp", "config": { "endpoint": "http://example.com" } }),
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
