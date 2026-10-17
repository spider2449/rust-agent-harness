//! The language servers: the table of configured servers, lazy start through
//! the `sandbox` service, and one check of one file.

use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use rah_api::{
    kernel::Spawner,
    sandbox::{ExecEnv, SpawnSpec},
};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::{
    ServerConfig,
    client::{Client, Diagnostic, Shared},
};

/// How long a server that could not start or ended is left alone.
const RETRY_AFTER: Duration = Duration::from_secs(300);
/// After a publication, how long to wait for the next one before the
/// diagnostics are taken as complete (servers publish in several steps).
const SETTLE: Duration = Duration::from_millis(250);

/// Servers that are tried when installed.
const DEFAULTS: [(&str, &[&str], &[&str]); 5] = [
    ("rust-analyzer", &["rust-analyzer"], &[".rs"]),
    (
        "typescript",
        &["typescript-language-server", "--stdio"],
        &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"],
    ),
    ("pyright", &["pyright-langserver", "--stdio"], &[".py"]),
    ("gopls", &["gopls"], &[".go"]),
    (
        "clangd",
        &["clangd"],
        &[".c", ".h", ".cc", ".cpp", ".cxx", ".hpp"],
    ),
];

pub(crate) struct Server {
    pub name: String,
    /// Whether the user wrote this server, so that a failure to start it is
    /// worth saying.
    pub explicit: bool,
    command: Vec<String>,
    /// Lowercase, with the leading dot.
    extensions: Vec<String>,
    env: BTreeMap<String, String>,
    initialization: Value,
    slot: Mutex<Slot>,
}

enum Slot {
    Idle,
    Ready(Arc<Connection>),
    Failed { at: Instant, message: String },
}

struct Connection {
    client: Client,
    /// One check at a time per server keeps the publications of different
    /// files apart.
    gate: Mutex<()>,
}

/// The outcome of one check.
pub(crate) enum Checked {
    Diagnostics(Vec<Diagnostic>),
    /// The server published nothing for the file in time.
    NoReport,
}

pub(crate) fn build(
    defaults: bool,
    configured: Option<&BTreeMap<String, ServerConfig>>,
) -> Result<Vec<Server>, String> {
    let mut table: BTreeMap<String, (ServerConfig, bool)> = BTreeMap::new();
    if defaults {
        for (name, command, extensions) in DEFAULTS {
            table.insert(
                name.to_owned(),
                (
                    ServerConfig {
                        command: Some(command.iter().map(|part| (*part).to_owned()).collect()),
                        extensions: Some(extensions.iter().map(|ext| (*ext).to_owned()).collect()),
                        ..ServerConfig::default()
                    },
                    false,
                ),
            );
        }
    }
    for (name, entry) in configured.into_iter().flatten() {
        if name.trim().is_empty() {
            return Err("a server needs a name".into());
        }
        if entry.disabled == Some(true) {
            table.remove(name);
            continue;
        }
        let (merged, _) = table.entry(name.clone()).or_default();
        if entry.command.is_some() {
            merged.command = entry.command.clone();
        }
        if entry.extensions.is_some() {
            merged.extensions = entry.extensions.clone();
        }
        if entry.env.is_some() {
            merged.env = entry.env.clone();
        }
        if entry.initialization.is_some() {
            merged.initialization = entry.initialization.clone();
        }
        if let Some(slot) = table.get_mut(name) {
            slot.1 = true;
        }
    }
    table
        .into_iter()
        .map(|(name, (config, explicit))| {
            let command = config
                .command
                .filter(|command| {
                    command
                        .first()
                        .is_some_and(|program| !program.trim().is_empty())
                })
                .ok_or_else(|| format!("server `{name}` needs a `command`"))?;
            let extensions: Vec<String> = config
                .extensions
                .filter(|extensions| !extensions.is_empty())
                .ok_or_else(|| format!("server `{name}` needs `extensions`"))?
                .into_iter()
                .map(|extension| extension.to_ascii_lowercase())
                .collect();
            if let Some(bad) = extensions
                .iter()
                .find(|extension| extension.len() < 2 || !extension.starts_with('.'))
            {
                return Err(format!(
                    "server `{name}`: extension `{bad}` must start with a dot"
                ));
            }
            if config
                .initialization
                .as_ref()
                .is_some_and(|value| !value.is_object())
            {
                return Err(format!(
                    "server `{name}`: `initialization` must be an object"
                ));
            }
            Ok(Server {
                name,
                explicit,
                command,
                extensions,
                env: config.env.unwrap_or_default(),
                initialization: config.initialization.unwrap_or(Value::Null),
                slot: Mutex::new(Slot::Idle),
            })
        })
        .collect()
}

pub(crate) struct Servers {
    servers: Vec<Server>,
    sandbox: Arc<dyn ExecEnv>,
    spawner: Spawner,
    root_uri: String,
    start_timeout: Duration,
}

impl Servers {
    pub fn new(
        servers: Vec<Server>,
        sandbox: Arc<dyn ExecEnv>,
        spawner: Spawner,
        root: &Path,
        start_timeout: Duration,
    ) -> Self {
        Self {
            servers,
            sandbox,
            spawner,
            root_uri: path_to_uri(root),
            start_timeout,
        }
    }

    /// The server that handles the extension of `path`.
    pub fn for_path(&self, path: &str) -> Option<&Server> {
        let extension = Path::new(path)
            .extension()?
            .to_string_lossy()
            .to_ascii_lowercase();
        self.servers.iter().find(|server| {
            server
                .extensions
                .iter()
                .any(|known| known[1..] == extension)
        })
    }

    /// Sends the new text of `file` to `server` and waits up to `wait` for
    /// the diagnostics it publishes.
    pub async fn check(
        &self,
        server: &Server,
        file: &Path,
        text: &str,
        wait: Duration,
    ) -> Result<Checked, String> {
        let connection = self.connection(server).await?;
        let _turn = connection.gate.lock().await;
        let uri = path_to_uri(file);
        let shared = &connection.client.shared;
        let baseline = shared.generation(&uri);
        let mut publications = shared.subscribe();
        publications.mark_unchanged();
        match shared.bump_version(&uri) {
            None => {
                connection
                    .client
                    .notify(
                        "textDocument/didOpen",
                        json!({ "textDocument": {
                            "uri": uri,
                            "languageId": language_id(file),
                            "version": 1,
                            "text": text,
                        } }),
                    )
                    .map_err(|error| error.to_string())?;
                1
            }
            Some(previous) => {
                let version = previous + 1;
                connection
                    .client
                    .notify(
                        "textDocument/didChange",
                        json!({
                            "textDocument": { "uri": uri, "version": version },
                            "contentChanges": [{ "text": text }],
                        }),
                    )
                    .map_err(|error| error.to_string())?;
                version
            }
        };
        // Some servers run their checker on save.
        connection
            .client
            .notify(
                "textDocument/didSave",
                json!({ "textDocument": { "uri": uri }, "text": text }),
            )
            .map_err(|error| error.to_string())?;

        let deadline = tokio::time::Instant::now() + wait;
        while shared.generation(&uri) == baseline {
            if tokio::time::timeout_at(deadline, publications.changed())
                .await
                .is_err()
                || connection.client.is_closed()
            {
                return Ok(Checked::NoReport);
            }
        }
        // Take the last of a burst of publications.
        while tokio::time::timeout(SETTLE, publications.changed())
            .await
            .is_ok_and(|changed| changed.is_ok())
        {}
        Ok(Checked::Diagnostics(shared.diagnostics(&uri)))
    }

    async fn connection(&self, server: &Server) -> Result<Arc<Connection>, String> {
        // Held while the server starts, so that two checks start it once.
        let mut slot = server.slot.lock().await;
        match &*slot {
            Slot::Ready(connection) if !connection.client.is_closed() => {
                return Ok(connection.clone());
            }
            Slot::Failed { at, message } if at.elapsed() < RETRY_AFTER => {
                return Err(message.clone());
            }
            _ => {}
        }
        match self.start(server).await {
            Ok(connection) => {
                *slot = Slot::Ready(connection.clone());
                Ok(connection)
            }
            Err(message) => {
                *slot = Slot::Failed {
                    at: Instant::now(),
                    message: message.clone(),
                };
                Err(message)
            }
        }
    }

    async fn start(&self, server: &Server) -> Result<Arc<Connection>, String> {
        let (program, args) = server
            .command
            .split_first()
            .ok_or_else(|| "no command".to_owned())?;
        let mut spec = SpawnSpec::new(program.clone(), args.to_vec());
        spec.env = server.env.clone();
        let spawned = self
            .sandbox
            .spawn(spec)
            .await
            .map_err(|error| error.to_string())?;
        let (client, actor) = Client::new(spawned, self.root_uri.clone(), Shared::new());
        if !self.spawner.spawn(actor) {
            return Err("the plugin is stopping".into());
        }
        let mut params = json!({
            "processId": std::process::id(),
            "rootUri": self.root_uri,
            "workspaceFolders": [{ "uri": self.root_uri, "name": "workspace" }],
            "clientInfo": { "name": "rah" },
            "capabilities": {
                "workspace": { "configuration": true, "workspaceFolders": true },
                "textDocument": {
                    "synchronization": { "didSave": true },
                    "publishDiagnostics": { "relatedInformation": false },
                },
                "window": { "workDoneProgress": true },
            },
        });
        if !server.initialization.is_null() {
            params["initializationOptions"] = server.initialization.clone();
        }
        if let Err(error) = client
            .request("initialize", params, self.start_timeout)
            .await
        {
            let stderr = client.stderr();
            let tail = stderr.trim();
            return Err(if tail.is_empty() {
                error.to_string()
            } else {
                format!("{error}: {}", tail.chars().take(300).collect::<String>())
            });
        }
        client
            .notify("initialized", json!({}))
            .map_err(|error| error.to_string())?;
        Ok(Arc::new(Connection {
            client,
            gate: Mutex::new(()),
        }))
    }
}

fn language_id(file: &Path) -> &'static str {
    match file
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("rs") => "rust",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("js" | "mjs" | "cjs") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("py") => "python",
        Some("go") => "go",
        Some("c" | "h") => "c",
        Some("cc" | "cpp" | "cxx" | "hpp") => "cpp",
        _ => "plaintext",
    }
}

/// A `file:` URI for an absolute path.
pub(crate) fn path_to_uri(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text
        .strip_prefix(r"\\?\")
        .unwrap_or(&text)
        .replace('\\', "/");
    let mut uri = String::from("file://");
    if !text.starts_with('/') {
        uri.push('/');
    }
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push_str(&format!("%{byte:02X}"));
        }
    }
    uri
}
