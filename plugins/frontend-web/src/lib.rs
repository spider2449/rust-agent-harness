//! `frontend-web`: a loopback HTTP and WebSocket API with an embedded Web UI.
//!
//! The server listens on a loopback address only (default
//! `127.0.0.1:8765`; port `0` picks a free one) and announces
//! `http://<address>/#token=<secret>` with
//! [`FrontendListening`](rah_api::host::FrontendListening). Every `/api`
//! request needs that per-start token (`Authorization: Bearer <token>`, or
//! `?token=` for the WebSocket), a `Host` naming this server, and, when
//! present, an `Origin` naming this server. The static UI needs no token; it
//! reads it from the URL fragment, which browsers never send.
//!
//! API:
//!
//! - `GET /api/info`: tools and whether a policy guards them.
//! - `GET /api/status`: the workspace root and its Git state (through the
//!   `sandbox` service), the model and loop the next turn uses, and whether
//!   a policy guards tools.
//! - With an `admin` service: `PUT /api/workspace {root}` points the
//!   workspace plugins at another directory. `GET /api/fs/dirs?path=` lists
//!   a directory's subdirectories (names, and whether each is a Git
//!   repository) for the folder picker.
//! - With a `sandbox` service: `GET /api/files?path=` lists one directory of
//!   the workspace and `GET /api/files/content?path=` returns the start of one
//!   file, for the read-only file browser. Paths are relative to the
//!   workspace root and cannot leave it. `GET /api/git/changes` and
//!   `/api/git/diff?path=` are read-only Git views of the workspace (one
//!   `git` run each, through the `sandbox` service).
//! - With an `admin` service: `GET|PUT /api/web-search {enabled}` turns
//!   web search on (the policy asks before `web.*`, then `tools-web-search`
//!   is mounted) or off, and remembers the choice in `storage` for the next
//!   start (see [`web_search`]). `PUT /api/settings/{slot} {id, kind,
//!   config}` saves the Settings forms (`model`, `assistant`, `tools`,
//!   `loop`) the same way, rolled back when the plugin does not start, and
//!   applies them again at the next start (see [`settings`]).
//!   `GET|PUT /api/preferences {raw_markdown}` keeps display preferences the
//!   same way.
//! - `GET /api/plugins`: plugin statuses, with kind and configuration when
//!   the distribution's `admin` service knows them.
//! - With an `admin` service: `GET /api/catalog`, `POST /api/plugins {kind,
//!   id, config, bind}`, `DELETE /api/plugins/{id}`,
//!   `POST /api/plugins/{id}/restart`, `POST /api/plugins/{id}/stop` and
//!   `/start` (stopped plugins are listed with the state `stopped`),
//!   `PUT /api/plugins/{id}/config
//!   {config}` and `POST /api/plugins/{id}/replace {kind, id, config, bind}`
//!   (mounts the replacement, then unmounts `{id}` once it is active).
//! - With a `scheduler` service: `GET|POST /api/jobs {input, session,
//!   delay_secs, every_secs}` and `DELETE /api/jobs/{id}`.
//! - `GET|POST /api/sessions`, `GET|DELETE /api/sessions/{id}`,
//!   `POST /api/sessions/{id}/fork {at}`, `PUT /api/sessions/{id}/title
//!   {title}` (`null` clears it) and `GET /api/sessions/{id}/export?format=md|json`
//!   (a download).
//! - `GET /api/ws`: WebSocket. Client messages: `{type: "turn", session,
//!   input}`, `{type: "cancel"}`, `{type: "approval", id, approve}`. Server
//!   messages: `{type: "event", session, event}`, `approval_required`,
//!   `approval_resolved`, `error`.
//!
//! The plugin answers `tool.approval` through connected browsers: with none
//! connected it does not answer (a denial), and an unanswered request is
//! denied after `approval_timeout_secs` (300).
//!
//! The loop and session services are resolved per request, so other plugins
//! can be swapped while the server runs. HTTP connections are served by
//! tasks of the HTTP library; WebSocket handlers end when this plugin's scope
//! is disposed.

mod access;
mod approvals;
mod export;
mod files;
mod git;
mod jobs;
mod manage;
mod settings;
mod status;
mod web_search;
mod workspace;
mod ws;

use std::{str::FromStr, sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use rah_api::{
    CancellationToken, KernelHandle, Plugin, PluginError, PluginId, PluginManifest, PluginStatus,
    Scope, async_trait,
    hooks::ToolApproval,
    host::FrontendListening,
    session::{SESSION, SessionError, SessionLog},
    tools::ToolInvoker,
};
use rah_protocol::SessionId;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::approvals::Approvals;

const INDEX_HTML: &str = include_str!("../assets/index.html");
const APP_JS: &str = include_str!("../assets/app.js");
const APP_CSS: &str = include_str!("../assets/app.css");
const SETTINGS_JS: &str = include_str!("../assets/settings.js");
const HIGHLIGHT_JS: &str = include_str!("../assets/highlight.js");
const MARKDOWN_JS: &str = include_str!("../assets/markdown.js");

pub struct WebFrontendPlugin {
    manifest: PluginManifest,
}

impl WebFrontendPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .optional(["loop", "session"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    bind: Option<String>,
    approval_timeout_secs: Option<u64>,
}

pub(crate) struct Shared {
    /// This plugin's id, also its `storage` namespace.
    pub id: PluginId,
    pub kernel: KernelHandle,
    pub token: String,
    pub hosts: Vec<String>,
    pub approvals: Approvals,
    /// Cancelled when the plugin scope is disposed.
    pub stop: CancellationToken,
}

pub(crate) type AppState = Arc<Shared>;

#[async_trait]
impl Plugin for WebFrontendPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let requested = access::loopback(config.bind.as_deref().unwrap_or("127.0.0.1:8765"))?;
        let listener = tokio::net::TcpListener::bind(requested)
            .await
            .map_err(|error| format!("cannot listen on {requested}: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("cannot read the listening address: {error}"))?;
        let timeout = Duration::from_secs(config.approval_timeout_secs.unwrap_or(300).max(1));
        let shared = Arc::new(Shared {
            id: scope.id().clone(),
            kernel: scope.kernel(),
            token: access::new_token(),
            hosts: access::allowed_hosts(address),
            approvals: Approvals::new(timeout),
            stop: scope.spawner().cancellation(),
        });
        let asker = shared.clone();
        scope.hook_bail::<ToolApproval, _, _>(0, move |request| {
            let asker = asker.clone();
            async move { asker.approvals.ask(request).await }
        });
        let app = router(shared.clone());
        let stop = shared.stop.clone();
        let started = scope.spawn(async move {
            let _ = axum::serve(listener, app)
                .with_graceful_shutdown(stop.cancelled_owned())
                .await;
        });
        if !started || !scope.spawn(settings::restore(shared.clone())) {
            return Err("the plugin scope is already disposed".into());
        }
        scope.kernel().emit(&FrontendListening {
            plugin: scope.id().clone(),
            url: format!("http://{address}/#token={}", shared.token),
        });
        Ok(())
    }
}

fn router(shared: AppState) -> Router {
    let api = Router::new()
        .route("/info", get(info))
        .route("/status", get(status::status))
        .route("/workspace", put(workspace::set))
        .route("/fs/dirs", get(workspace::list))
        .route("/files", get(files::list))
        .route("/files/content", get(files::content))
        .route("/git/changes", get(git::changes))
        .route("/git/diff", get(git::diff))
        .route("/jobs", get(jobs::list).post(jobs::submit))
        .route("/jobs/{id}", axum::routing::delete(jobs::cancel))
        .route("/web-search", get(web_search::get).put(web_search::set))
        .route("/settings/{slot}", put(settings::save))
        .route(
            "/preferences",
            get(settings::preferences).put(settings::set_preferences),
        )
        .route("/catalog", get(manage::catalog))
        .route("/plugins", get(manage::list).post(manage::mount))
        .route("/plugins/{id}", axum::routing::delete(manage::unmount))
        .route("/plugins/{id}/restart", post(manage::restart))
        .route("/plugins/{id}/stop", post(manage::stop))
        .route("/plugins/{id}/start", post(manage::start))
        .route("/plugins/{id}/config", put(manage::configure))
        .route("/plugins/{id}/replace", post(manage::replace))
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{id}", get(read_session).delete(delete_session))
        .route("/sessions/{id}/fork", post(fork_session))
        .route("/sessions/{id}/title", put(rename_session))
        .route("/sessions/{id}/export", get(export_session))
        .route("/ws", get(ws::upgrade))
        .layer(middleware::from_fn_with_state(
            shared.clone(),
            require_token,
        ));
    Router::new()
        .route(
            "/",
            get(|| async { asset("text/html; charset=utf-8", INDEX_HTML) }),
        )
        .route(
            "/app.js",
            get(|| async { asset("text/javascript; charset=utf-8", APP_JS) }),
        )
        .route(
            "/settings.js",
            get(|| async { asset("text/javascript; charset=utf-8", SETTINGS_JS) }),
        )
        .route(
            "/highlight.js",
            get(|| async { asset("text/javascript; charset=utf-8", HIGHLIGHT_JS) }),
        )
        .route(
            "/markdown.js",
            get(|| async { asset("text/javascript; charset=utf-8", MARKDOWN_JS) }),
        )
        .route(
            "/app.css",
            get(|| async { asset("text/css; charset=utf-8", APP_CSS) }),
        )
        .nest("/api", api)
        .layer(middleware::from_fn_with_state(shared.clone(), require_host))
        .with_state(shared)
}

fn asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'self'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
        ],
        body,
    )
        .into_response()
}

fn header_str(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

async fn require_host(State(shared): State<AppState>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    if !access::host_allowed(&shared.hosts, header_str(headers, header::HOST))
        || !access::origin_allowed(&shared.hosts, header_str(headers, header::ORIGIN))
    {
        return (StatusCode::FORBIDDEN, "forbidden host or origin").into_response();
    }
    next.run(request).await
}

async fn require_token(State(shared): State<AppState>, request: Request, next: Next) -> Response {
    let bearer = header_str(request.headers(), header::AUTHORIZATION)
        .and_then(|value| value.strip_prefix("Bearer "));
    let query = request.uri().query().and_then(|query| {
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix("token="))
    });
    match bearer.or(query) {
        Some(token) if access::token_matches(&shared.token, token) => next.run(request).await,
        _ => (StatusCode::UNAUTHORIZED, "a valid token is required").into_response(),
    }
}

/// An API error as `{error}` with a status code.
pub(crate) struct ApiError(pub StatusCode, pub String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<SessionError> for ApiError {
    fn from(error: SessionError) -> Self {
        let status = match error {
            SessionError::NotFound(_) => StatusCode::NOT_FOUND,
            SessionError::InvalidFork { .. } | SessionError::InvalidTitle => {
                StatusCode::BAD_REQUEST
            }
            SessionError::HasForks { .. } => StatusCode::CONFLICT,
            SessionError::Unsupported(_) => StatusCode::NOT_IMPLEMENTED,
            SessionError::Corrupt { .. } | SessionError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self(status, error.to_string())
    }
}

pub(crate) type ApiResult = Result<Json<Value>, ApiError>;

fn sessions(shared: &Shared) -> Result<Arc<dyn SessionLog>, ApiError> {
    shared.kernel.get(SESSION).ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "no session service is mounted".into(),
        )
    })
}

fn session_id(id: &str) -> Result<SessionId, ApiError> {
    SessionId::from_str(id).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            format!("`{id}` is not a session id"),
        )
    })
}

fn to_json(value: impl serde::Serialize) -> ApiResult {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|error| ApiError(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))
}

async fn info(State(shared): State<AppState>) -> ApiResult {
    let invoker = ToolInvoker::new(shared.kernel.clone());
    Ok(Json(json!({
        "guarded": invoker.is_guarded(),
        "tools": invoker.definitions(),
        "loop": shared.kernel.get(rah_api::agent_loop::LOOP).is_some(),
        "session": shared.kernel.get(SESSION).is_some(),
    })))
}

pub(crate) fn status_json(status: &PluginStatus) -> Value {
    let names = |items: &[String]| items.join(", ");
    match status {
        PluginStatus::Pending { missing } => json!({
            "state": "pending",
            "detail": names(&missing.iter().map(ToString::to_string).collect::<Vec<_>>()),
        }),
        PluginStatus::DependencyCycle { cycle } => json!({
            "state": "dependency_cycle",
            "detail": names(&cycle.iter().map(ToString::to_string).collect::<Vec<_>>()),
        }),
        PluginStatus::Failed { message } => json!({ "state": "failed", "detail": message }),
        other => json!({ "state": format!("{other:?}").to_lowercase(), "detail": "" }),
    }
}

async fn list_sessions(State(shared): State<AppState>) -> ApiResult {
    to_json(sessions(&shared)?.list().await?)
}

async fn create_session(State(shared): State<AppState>) -> ApiResult {
    to_json(sessions(&shared)?.create().await?)
}

async fn read_session(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    to_json(sessions(&shared)?.read(&session_id(&id)?).await?)
}

#[derive(Deserialize)]
struct Fork {
    at: u64,
}

async fn fork_session(
    State(shared): State<AppState>,
    Path(id): Path<String>,
    Json(fork): Json<Fork>,
) -> ApiResult {
    to_json(sessions(&shared)?.fork(&session_id(&id)?, fork.at).await?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    title: Option<String>,
}

async fn rename_session(
    State(shared): State<AppState>,
    Path(id): Path<String>,
    Json(rename): Json<Rename>,
) -> ApiResult {
    to_json(
        sessions(&shared)?
            .rename(&session_id(&id)?, rename.title)
            .await?,
    )
}

async fn delete_session(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    sessions(&shared)?.delete(&session_id(&id)?).await?;
    Ok(Json(json!({ "id": id, "deleted": true })))
}

#[derive(Deserialize)]
struct Export {
    format: Option<String>,
}

/// `GET /api/sessions/{id}/export?format=md|json`, as a download.
async fn export_session(
    State(shared): State<AppState>,
    Path(id): Path<String>,
    axum::extract::Query(export): axum::extract::Query<Export>,
) -> Result<Response, ApiError> {
    let id = session_id(&id)?;
    let log = sessions(&shared)?;
    let records = log.read(&id).await?;
    let info = log
        .list()
        .await?
        .into_iter()
        .find(|info| info.id == id)
        .ok_or_else(|| SessionError::NotFound(id.clone()))?;
    let (body, content_type, extension) = match export.format.as_deref().unwrap_or("md") {
        "md" | "markdown" => (
            export::markdown(&info, &records),
            "text/markdown; charset=utf-8",
            "md",
        ),
        "json" => (
            export::json(&info, &records),
            "application/json; charset=utf-8",
            "json",
        ),
        other => {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                format!("unknown export format `{other}`; use md or json"),
            ));
        }
    };
    // The file name uses only the session id, never the (untrusted) title.
    let disposition = format!("attachment; filename=\"rah-session-{id}.{extension}\"");
    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_owned()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CACHE_CONTROL, "no-store".to_owned()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        body,
    )
        .into_response())
}
