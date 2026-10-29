//! Engine failover against scripted engines on a loopback port.

use std::sync::Mutex as StdMutex;

use rah_api::PluginStatus;
use rah_protocol::SessionId;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::*;

const DDG_PAGE: &str = r#"<a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fddg.example%2F">DDG hit</a>
<a class="result__snippet" href="x">from duckduckgo</a>
<a class="result__a" href="https://ddg.example">Duplicate</a>
<a class="result__a" href="https://second.example/">Second</a>"#;
const BING_PAGE: &str = r#"<li class="b_algo"><h2><a href="https://bing.example/">Bing hit</a></h2><p>from bing</p></li>"#;
const WALL_PAGE: &str = r#"<div class="anomaly-modal">Please verify</div>"#;

/// The fixture's answer for one path.
#[derive(Clone)]
enum Reply {
    Page(u16, &'static str),
    Json(Value),
    /// Status, content type and body.
    Typed(u16, &'static str, String),
    /// Content type and raw body bytes, with status 200.
    Bytes(&'static str, Vec<u8>),
    /// A `302` to this location.
    Redirect(String),
    /// Never answers.
    Hold,
}

struct Fixture {
    base: String,
    /// Request lines (`GET /path?query`) in arrival order.
    seen: Arc<StdMutex<Vec<String>>>,
}

/// Serves `routes` (path prefix to reply) until the test ends.
async fn fixture(routes: Vec<(&'static str, Reply)>) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(StdMutex::new(Vec::new()));
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let routes = routes.clone();
            let log = log.clone();
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buffer).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => request.extend_from_slice(&buffer[..read]),
                    }
                }
                let request = String::from_utf8_lossy(&request).into_owned();
                let line = request.lines().next().unwrap_or_default().to_owned();
                let path = line.split(' ').nth(1).unwrap_or_default().to_owned();
                log.lock().unwrap().push(line);
                let reply = routes
                    .iter()
                    .find(|(prefix, _)| path.starts_with(prefix))
                    .map(|(_, reply)| reply.clone())
                    .unwrap_or(Reply::Page(404, "missing"));
                let mut extra = String::new();
                let (status, content_type, body) = match reply {
                    Reply::Page(status, body) => (status, "text/html", body.as_bytes().to_vec()),
                    Reply::Json(body) => (200, "application/json", body.to_string().into_bytes()),
                    Reply::Typed(status, content_type, body) => {
                        (status, content_type, body.into_bytes())
                    }
                    Reply::Bytes(content_type, body) => (200, content_type, body),
                    Reply::Redirect(location) => {
                        extra = format!("location: {location}\r\n");
                        (302, "text/plain", Vec::new())
                    }
                    Reply::Hold => {
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        return;
                    }
                };
                let response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n",
                    body.len()
                );
                let mut response = response.into_bytes();
                response.extend_from_slice(&body);
                let _ = stream.write_all(&response).await;
            });
        }
    });
    Fixture { base, seen }
}

impl Fixture {
    fn tool(&self, extra: Value) -> WebSearch {
        let mut config = json!({
            "duckduckgo_url": format!("{}/ddg", self.base),
            "bing_url": format!("{}/bing", self.base),
            "timeout_secs": 5
        });
        if let (Value::Object(config), Value::Object(extra)) = (&mut config, extra) {
            config.extend(extra);
        }
        WebSearch::from_config(serde_json::from_value(config).unwrap()).unwrap()
    }

    fn seen(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }
}

fn cx() -> ToolContext {
    ToolContext {
        session: SessionId::new(),
        call: rah_protocol::ToolCallId::new(),
        cancel: CancellationToken::new(),
    }
}

async fn search(tool: &WebSearch, input: Value) -> Result<String, ToolError> {
    let output = tool.call(input, cx()).await?;
    match output.content.as_slice() {
        [ToolContent::Text(text)] => Ok(text.clone()),
        other => panic!("unexpected output {other:?}"),
    }
}

#[tokio::test]
async fn first_engine_answers_with_wrapped_deduplicated_results() {
    let fixture = fixture(vec![
        ("/ddg", Reply::Page(200, DDG_PAGE)),
        ("/bing", Reply::Page(200, BING_PAGE)),
    ])
    .await;
    let tool = fixture.tool(json!({}));
    let text = search(&tool, json!({ "query": "  ddg   hit & more " }))
        .await
        .unwrap();
    assert_eq!(
        text,
        "Note: results from duckduckgo.\n\
         <untrusted-web-content engine=\"duckduckgo\">\n\
         1. DDG hit\n   https://ddg.example/\n   from duckduckgo\n\
         2. Second\n   https://second.example/\n\
         </untrusted-web-content>"
    );
    assert_eq!(
        fixture.seen(),
        vec!["GET /ddg?q=ddg%20hit%20%26%20more HTTP/1.1"]
    );

    let text = search(&tool, json!({ "query": "hit", "limit": 1 }))
        .await
        .unwrap();
    assert!(!text.contains("Second"), "{text}");
}

#[tokio::test]
async fn blocked_and_rate_limited_engines_fall_over_and_cool_down() {
    let fixture = fixture(vec![
        ("/ddg", Reply::Page(200, WALL_PAGE)),
        ("/bing", Reply::Page(429, "slow down")),
        (
            "/searx",
            Reply::Json(json!({ "results": [
                { "url": "https://searx.example/", "title": "Searx hit", "content": "from searxng" }
            ]})),
        ),
    ])
    .await;
    let tool = fixture.tool(json!({ "searxng": [format!("{}/searx", fixture.base)] }));
    let text = search(&tool, json!({ "query": "q" })).await.unwrap();
    let host = host(&fixture.base).to_owned();
    assert!(
        text.starts_with(&format!(
            "Note: duckduckgo failed (blocked: bot check page); bing failed (quota: HTTP 429 Too Many Requests); using searxng({host})."
        )),
        "{text}"
    );
    assert!(text.contains("1. Searx hit\n   https://searx.example/\n   from searxng"));

    // Both failed engines are skipped while they cool down.
    let text = search(&tool, json!({ "query": "q" })).await.unwrap();
    assert!(
        text.starts_with("Note: duckduckgo is cooling down; bing is cooling down;"),
        "{text}"
    );
    assert_eq!(fixture.seen().len(), 4);

    // Naming an engine bypasses its cooldown and disables failover.
    let error = search(&tool, json!({ "query": "q", "engine": "bing" }))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ToolError::Execution(message) if message.contains("bing failed (quota")),
        "{error:?}"
    );
}

#[tokio::test]
async fn transient_and_invalid_failures_do_not_cool_down() {
    let fixture = fixture(vec![
        ("/ddg", Reply::Page(503, "down")),
        ("/bing", Reply::Page(200, BING_PAGE)),
        ("/searx", Reply::Page(200, "<html>not json</html>")),
    ])
    .await;
    let tool = fixture.tool(json!({
        "engines": ["searxng", "duckduckgo", "bing"],
        "searxng": [format!("{}/searx/", fixture.base)]
    }));
    for _ in 0..2 {
        let text = search(&tool, json!({ "query": "q" })).await.unwrap();
        assert!(
            text.contains("invalid-response: not a SearXNG JSON response"),
            "{text}"
        );
        assert!(
            text.contains("duckduckgo failed (transient: HTTP 503"),
            "{text}"
        );
        assert!(text.contains("1. Bing hit"), "{text}");
    }
    let seen = fixture.seen();
    assert_eq!(seen.len(), 6);
    assert_eq!(seen[0], "GET /searx/search?q=q&format=json HTTP/1.1");
}

#[tokio::test]
async fn empty_results_and_total_failure_are_reported() {
    let fixture = fixture(vec![
        ("/ddg", Reply::Page(200, "<html>nothing</html>")),
        ("/bing", Reply::Page(403, "no")),
    ])
    .await;
    let tool = fixture.tool(json!({ "bing_market": "zh-CN" }));
    let text = search(&tool, json!({ "query": "q" })).await.unwrap();
    assert_eq!(
        text,
        "Note: bing failed (blocked: HTTP 403 Forbidden).\nNo results (duckduckgo returned none)."
    );
    assert!(fixture.seen()[1].starts_with("GET /bing?q=q&setmkt=zh-CN "));

    let tool = fixture.tool(json!({ "engines": ["bing"] }));
    assert!(matches!(
        search(&tool, json!({ "query": "q" })).await,
        Err(ToolError::Execution(message)) if message.starts_with("every search engine failed")
    ));
}

#[tokio::test]
async fn cancellation_stops_a_pending_request() {
    let fixture = fixture(vec![("/ddg", Reply::Hold)]).await;
    let tool = fixture.tool(json!({ "engines": ["duckduckgo"] }));
    let cx = cx();
    let cancel = cx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
    });
    assert!(matches!(
        tool.call(json!({ "query": "q" }), cx).await,
        Err(ToolError::Cancelled)
    ));
}

#[tokio::test]
async fn input_and_configuration_are_validated() {
    let fixture = fixture(vec![]).await;
    let tool = fixture.tool(json!({}));
    for input in [
        json!({}),
        json!({ "query": "   " }),
        json!({ "query": "x".repeat(MAX_QUERY_CHARS + 1) }),
        json!({ "query": "q", "limit": 0 }),
        json!({ "query": "q", "limit": MAX_RESULTS + 1 }),
        json!({ "query": "q", "engine": "searxng" }),
        json!({ "query": "q", "extra": true }),
    ] {
        assert!(
            matches!(
                search(&tool, input.clone()).await,
                Err(ToolError::InvalidInput(_))
            ),
            "{input}"
        );
    }
    assert!(fixture.seen().is_empty());

    let definition = tool.definition();
    assert_eq!(definition.name, ToolName::new("web.search"));
    assert_eq!(definition.permission, PermissionLevel::Read);
    assert_eq!(
        definition.input_schema["properties"]["engine"]["enum"],
        json!(["duckduckgo", "bing"])
    );

    for config in [
        json!({ "engines": [] }),
        json!({ "engines": ["google"] }),
        json!({ "engines": ["searxng"] }),
        json!({ "searxng": ["file:///etc"] }),
        json!({ "bing_url": "ftp://x" }),
        json!({ "max_results": 0 }),
        json!({ "max_results": 21 }),
        json!({ "proxy": "not a url" }),
        json!({ "unknown": 1 }),
    ] {
        let parsed = serde_json::from_value::<Config>(config.clone());
        assert!(
            parsed.map_or(true, |config| WebSearch::from_config(config).is_err()),
            "{config}"
        );
    }
}

#[tokio::test]
async fn the_plugin_mounts_and_serves_through_the_invoker() {
    use rah_api::{
        Kernel, MountOptions,
        tools::{Invocation, ToolInvoker},
    };
    use rah_protocol::{ToolCall, ToolCallId, ToolInput};

    let fixture = fixture(vec![("/bing", Reply::Page(200, BING_PAGE))]).await;
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(WebSearchPlugin::new("tools-web-search")),
            MountOptions::with_config(json!({
                "engines": ["bing"],
                "bing_url": format!("{}/bing", fixture.base)
            })),
        )
        .await
        .unwrap();
    assert_eq!(status, PluginStatus::Active);
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("web.search"),
                    input: ToolInput(json!({ "query": "q" })),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
        .await;
    assert!(!outcome.output.is_error, "{:?}", outcome.output);
    assert!(
        matches!(&outcome.output.content[0], ToolContent::Text(text) if text.contains("1. Bing hit"))
    );

    let status = kernel
        .mount(
            Arc::new(WebSearchPlugin::new("bad-search")),
            MountOptions::with_config(json!({ "engines": ["google"] })),
        )
        .await
        .unwrap();
    assert!(matches!(status, PluginStatus::Failed { .. }), "{status:?}");
}

#[tokio::test]
async fn time_range_maps_to_each_engine_parameter() {
    let fixture = fixture(vec![
        ("/ddg", Reply::Page(200, "<html>nothing</html>")),
        ("/bing", Reply::Page(200, "<html>nothing</html>")),
        ("/searx", Reply::Json(json!({ "results": [] }))),
    ])
    .await;
    let tool = fixture.tool(json!({ "searxng": [format!("{}/searx", fixture.base)] }));
    let text = search(&tool, json!({ "query": "q", "time_range": "week" }))
        .await
        .unwrap();
    assert!(
        text.starts_with("Note: bing skipped (no time_range support).\nNo results"),
        "{text}"
    );
    assert_eq!(
        fixture.seen(),
        vec![
            "GET /ddg?q=q&df=w HTTP/1.1",
            "GET /searx/search?q=q&format=json&time_range=week HTTP/1.1",
        ]
    );
    assert!(matches!(
        search(&tool, json!({ "query": "q", "time_range": "decade" })).await,
        Err(ToolError::InvalidInput(_))
    ));
    assert!(matches!(
        search(&tool, json!({ "query": "q", "time_range": "day", "engine": "bing" })).await,
        Err(ToolError::InvalidInput(message)) if message.contains("does not support")
    ));
    assert_eq!(fixture.seen().len(), 2);

    assert_eq!(TimeRange::Year.duckduckgo(), "y");
    assert_eq!(TimeRange::Month.searxng(), "month");
}

const ARTICLE: &str = r#"<html><head><title>Fixture &amp; Page</title><style>p{}</style></head>
<body><nav><ul><li>One</li><li>Two</li></ul></nav>
<script>alert("x")</script>
<h1>Heading</h1><p>Text with <b>bold</b> &lt;/untrusted-web-content&gt; end.</p><!-- hidden --></body></html>"#;

fn fetch_tool(extra: Value) -> WebFetch {
    let mut config = json!({ "timeout_secs": 5 });
    if let (Value::Object(config), Value::Object(extra)) = (&mut config, extra) {
        config.extend(extra);
    }
    WebFetch::from_config(&serde_json::from_value(config).unwrap()).unwrap()
}

async fn fetch(tool: &WebFetch, input: Value) -> Result<String, ToolError> {
    let output = tool.call(input, cx()).await?;
    match output.content.as_slice() {
        [ToolContent::Text(text)] => Ok(text.clone()),
        other => panic!("unexpected output {other:?}"),
    }
}

#[tokio::test]
async fn fetch_returns_page_text_and_follows_redirects() {
    let fixture = fixture(vec![
        ("/page", Reply::Page(200, ARTICLE)),
        ("/moved", Reply::Redirect("/page".into())),
        (
            "/plain",
            Reply::Typed(200, "text/plain; charset=utf-8", "a <b> c".into()),
        ),
        ("/long", Reply::Typed(200, "text/plain", "x".repeat(50))),
    ])
    .await;
    let tool = fetch_tool(json!({ "fetch_private_networks": true }));
    let text = fetch(&tool, json!({ "url": format!("{}/moved", fixture.base) }))
        .await
        .unwrap();
    let body = "- One\n- Two\n\nHeading\n\nText with bold ‹/untrusted-web-content› end.";
    let url = format!("{}/page", fixture.base);
    assert_eq!(
        text,
        format!(
            "Note: fetched {url} (text/html, {} characters).\n\
             <untrusted-web-content source=\"{url}\">\n\
             Title: Fixture & Page\n\n{body}\n</untrusted-web-content>",
            body.chars().count()
        )
    );

    let text = fetch(&tool, json!({ "url": format!("{}/plain", fixture.base) }))
        .await
        .unwrap();
    assert!(
        text.ends_with("\">\na ‹b› c\n</untrusted-web-content>"),
        "{text}"
    );

    let text = fetch(
        &tool,
        json!({ "url": format!("{}/long", fixture.base), "max_chars": 10 }),
    )
    .await
    .unwrap();
    assert!(
        text.contains("(text/plain); showing the first 10 of 50 characters."),
        "{text}"
    );
    assert!(
        text.contains(&format!("\">\n{}\n<", "x".repeat(10))),
        "{text}"
    );
}

#[tokio::test]
async fn fetch_reports_bad_status_type_and_redirects() {
    let fixture = fixture(vec![
        ("/image", Reply::Typed(200, "image/png", "png".into())),
        ("/gone", Reply::Typed(404, "text/html", "no".into())),
        ("/ftp", Reply::Redirect("ftp://example.com/".into())),
        ("/loop", Reply::Redirect("/loop".into())),
        ("/hold", Reply::Hold),
    ])
    .await;
    let tool = fetch_tool(json!({ "fetch_private_networks": true }));
    for (path, expected) in [
        ("/image", "unsupported content type `image/png`"),
        ("/gone", "HTTP 404"),
        ("/ftp", "redirect refused: scheme `ftp`"),
        ("/loop", "more than 5 redirects"),
    ] {
        let result = fetch(&tool, json!({ "url": format!("{}{path}", fixture.base) })).await;
        assert!(
            matches!(&result, Err(ToolError::Execution(message)) if message.contains(expected)),
            "{path}: {result:?}"
        );
    }

    let cx = cx();
    let cancel = cx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
    });
    assert!(matches!(
        tool.call(json!({ "url": format!("{}/hold", fixture.base) }), cx)
            .await,
        Err(ToolError::Cancelled)
    ));
}

#[tokio::test]
async fn fetch_refuses_private_addresses_by_default() {
    let fixture = fixture(vec![("/", Reply::Page(200, ARTICLE))]).await;
    let port = fixture.base.rsplit(':').next().unwrap().to_owned();
    let tool = fetch_tool(json!({}));
    for url in [
        fixture.base.clone(),
        format!("http://2130706433:{port}/"),
        "http://[::1]/".to_owned(),
        "http://[::ffff:10.0.0.1]/".to_owned(),
        "http://169.254.169.254/latest/meta-data/".to_owned(),
        "http://user:secret@example.com/".to_owned(),
        "file:///etc/passwd".to_owned(),
        "not a url".to_owned(),
    ] {
        assert!(
            matches!(
                fetch(&tool, json!({ "url": url })).await,
                Err(ToolError::InvalidInput(_))
            ),
            "{url}"
        );
    }
    // A name is checked after resolution.
    let result = fetch(&tool, json!({ "url": format!("http://localhost:{port}/") })).await;
    assert!(
        matches!(&result, Err(ToolError::Execution(message)) if message.contains("does not resolve to a public address")),
        "{result:?}"
    );
    assert!(fixture.seen().is_empty());

    for input in [
        json!({ "url": "https://example.com/", "max_chars": 0 }),
        json!({ "url": "https://example.com/", "max_chars": 20_001 }),
        json!({ "url": format!("https://example.com/{}", "a".repeat(2_000)) }),
        json!({ "url": "https://example.com/", "extra": 1 }),
    ] {
        assert!(matches!(
            fetch(&tool, input).await,
            Err(ToolError::InvalidInput(_))
        ));
    }
    for config in [
        json!({ "fetch_max_chars": 0 }),
        json!({ "fetch_max_chars": 100_001 }),
    ] {
        assert!(WebFetch::from_config(&serde_json::from_value(config).unwrap()).is_err());
    }
    let definition = tool.definition();
    assert_eq!(definition.name, ToolName::new("web.fetch"));
    assert_eq!(definition.permission, PermissionLevel::Read);
}

#[test]
fn only_public_addresses_pass_the_guard() {
    use crate::fetch::{is_public, public_addrs};
    use std::net::{IpAddr, SocketAddr};

    for public in ["8.8.8.8", "1.1.1.1", "2606:4700::1111", "64:ff9b::808:808"] {
        assert!(is_public(public.parse::<IpAddr>().unwrap()), "{public}");
    }
    for private in [
        "0.0.0.0",
        "10.1.2.3",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "192.0.0.8",
        "192.0.2.1",
        "192.168.1.1",
        "198.18.0.1",
        "224.0.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "::ffff:127.0.0.1",
        "::127.0.0.1",
        "64:ff9b::a00:1",
        "fc00::1",
        "fd12::1",
        "fe80::1",
        "ff02::1",
        "2001:db8::1",
    ] {
        assert!(!is_public(private.parse::<IpAddr>().unwrap()), "{private}");
    }
    let addr = |text: &str| text.parse::<SocketAddr>().unwrap();
    assert_eq!(
        public_addrs(vec![addr("8.8.8.8:0"), addr("1.1.1.1:0")]),
        vec![addr("8.8.8.8:0"), addr("1.1.1.1:0")]
    );
    assert!(public_addrs(vec![addr("8.8.8.8:0"), addr("10.0.0.1:0")]).is_empty());
}

fn multi(tool: WebSearch) -> MultiSearch {
    MultiSearch(Arc::new(tool))
}

async fn multi_search(tool: &MultiSearch, input: Value) -> Result<String, ToolError> {
    let output = tool.call(input, cx()).await?;
    match output.content.as_slice() {
        [ToolContent::Text(text)] => Ok(text.clone()),
        other => panic!("unexpected output {other:?}"),
    }
}

#[tokio::test]
async fn multi_search_merges_engines_and_ranks_agreement_first() {
    let fixture = fixture(vec![
        (
            "/ddg",
            Reply::Page(
                200,
                r#"<a class="result__a" href="https://a.example/">A</a>
<a class="result__a" href="https://shared.example/x">Shared</a>
<a class="result__snippet" href="s">ddg snippet</a>"#,
            ),
        ),
        (
            "/bing",
            Reply::Page(
                200,
                r#"<li class="b_algo"><h2><a href="https://SHARED.example/x/">Shared</a></h2></li>
<li class="b_algo"><h2><a href="https://c.example/">C</a></h2><p>c snippet</p></li>"#,
            ),
        ),
        ("/searx", Reply::Page(429, "slow down")),
    ])
    .await;
    let tool = multi(fixture.tool(json!({ "searxng": [format!("{}/searx", fixture.base)] })));
    let host = host(&fixture.base).to_owned();
    let text = multi_search(&tool, json!({ "query": "q" })).await.unwrap();
    assert_eq!(
        text,
        format!(
            "Note: results from duckduckgo, bing; searxng({host}) failed (quota: HTTP 429 Too Many Requests).\n\
             <untrusted-web-content engine=\"multi\">\n\
             1. Shared\n   https://SHARED.example/x/\n   ddg snippet\n   seen in: bing, duckduckgo\n\
             2. A\n   https://a.example/\n   seen in: duckduckgo\n\
             3. C\n   https://c.example/\n   c snippet\n   seen in: bing\n\
             </untrusted-web-content>"
        )
    );

    // The failed engine cools down; `limit` cuts the merged list.
    let text = multi_search(&tool, json!({ "query": "q", "limit": 1 }))
        .await
        .unwrap();
    assert!(
        text.contains(&format!("searxng({host}) is cooling down")),
        "{text}"
    );
    assert!(
        text.contains("1. Shared") && !text.contains("2. "),
        "{text}"
    );

    // Listing engines bypasses cooldowns and drops the others.
    let error = multi_search(&tool, json!({ "query": "q", "engines": ["searxng"] }))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ToolError::Execution(message) if message.contains("quota")),
        "{error:?}"
    );
    assert!(matches!(
        multi_search(
            &tool,
            json!({ "query": "q", "engines": ["bing"], "time_range": "day" }),
        )
        .await,
        Err(ToolError::InvalidInput(message)) if message.contains("time_range")
    ));
    // A time range skips Bing; the cooling SearXNG is not requested either.
    let before = fixture.seen().len();
    let text = multi_search(&tool, json!({ "query": "q", "time_range": "day" }))
        .await
        .unwrap();
    assert!(
        text.starts_with(&format!(
            "Note: results from duckduckgo; bing skipped (no time_range support); searxng({host}) is cooling down."
        )),
        "{text}"
    );
    let seen = fixture.seen();
    assert_eq!(seen.len(), before + 1);
    assert_eq!(seen[before], "GET /ddg?q=q&df=d HTTP/1.1");
    for engines in [json!([]), json!(["google"])] {
        assert!(matches!(
            multi_search(&tool, json!({ "query": "q", "engines": engines })).await,
            Err(ToolError::InvalidInput(_))
        ));
    }
}

#[tokio::test]
async fn multi_search_reports_empty_engines_and_cancellation() {
    let fixture = fixture(vec![
        ("/ddg", Reply::Page(200, "<html>nothing</html>")),
        ("/bing", Reply::Hold),
    ])
    .await;
    let tool = multi(fixture.tool(json!({})));
    let text = multi_search(&tool, json!({ "query": "q", "engines": ["duckduckgo"] }))
        .await
        .unwrap();
    assert_eq!(text, "No results (duckduckgo returned none).");

    let bing_only = multi(fixture.tool(json!({ "engines": ["bing"] })));
    assert!(matches!(
        multi_search(&bing_only, json!({ "query": "q", "time_range": "week" })).await,
        Err(ToolError::Execution(message))
            if message == "no search engine is available: bing skipped (no time_range support)"
    ));

    let cx = cx();
    let cancel = cx.cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel.cancel();
    });
    assert!(matches!(
        tool.call(json!({ "query": "q" }), cx).await,
        Err(ToolError::Cancelled)
    ));
    assert_eq!(tool.definition().name, ToolName::new("web.multi_search"));
}

#[tokio::test]
async fn fetch_decodes_declared_charsets() {
    let (big5, _, _) = encoding_rs::BIG5.encode("中文測試");
    let mut meta_page = b"<html><head><meta charset=\"big5\"><title>".to_vec();
    meta_page.extend_from_slice(&big5);
    meta_page.extend_from_slice(b"</title></head><body><p>");
    meta_page.extend_from_slice(&big5);
    meta_page.extend_from_slice(b"</p></body></html>");
    let fixture = fixture(vec![
        (
            "/header",
            Reply::Bytes("text/plain; charset=Big5", big5.to_vec()),
        ),
        ("/meta", Reply::Bytes("text/html", meta_page)),
        (
            "/latin",
            Reply::Bytes("text/plain; charset=iso-8859-1", b"caf\xe9".to_vec()),
        ),
        (
            "/unknown",
            Reply::Bytes("text/plain; charset=nonsense", "ok".into()),
        ),
    ])
    .await;
    let tool = fetch_tool(json!({ "fetch_private_networks": true }));
    let get = |path: &str| fetch(&tool, json!({ "url": format!("{}{path}", fixture.base) }));

    let text = get("/header").await.unwrap();
    assert!(text.contains("(text/plain, Big5, 4 characters)"), "{text}");
    assert!(text.contains("\">\n中文測試\n<"), "{text}");
    let text = get("/meta").await.unwrap();
    assert!(text.contains("(text/html, Big5,"), "{text}");
    assert!(text.contains("Title: 中文測試\n\n中文測試\n<"), "{text}");
    let text = get("/latin").await.unwrap();
    assert!(text.contains("\">\ncafé\n<"), "{text}");
    let text = get("/unknown").await.unwrap();
    assert!(text.contains("(text/plain, 2 characters)"), "{text}");
}

#[tokio::test]
async fn unrelated_scraped_results_fall_over() {
    let fixture = fixture(vec![
        ("/bing", Reply::Page(200, r#"<li class="b_algo"><h2><a href="https://vietnam.travel/">Vietnamese dishes</a></h2></li>"#)),
        ("/ddg", Reply::Page(200, r#"<a class="result__a" href="https://docs.rs/tokio-util/">CancellationToken</a>"#)),
    ])
    .await;
    let tool = fixture.tool(json!({ "engines": ["bing", "duckduckgo"] }));
    let text = search(&tool, json!({ "query": "tokio cancellation" }))
        .await
        .unwrap();
    assert!(
        text.starts_with(
            "Note: bing failed (invalid-response: results do not match the query); using duckduckgo."
        ),
        "{text}"
    );
    assert!(text.contains("1. CancellationToken"), "{text}");
}
