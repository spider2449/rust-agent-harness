const applicationRows = [
  ["Version", "appVersion"],
  ["Platform", "platform"],
  ["Desktop shell", "desktopShell"],
];

const runtimeRows = [
  ["RAH Runtime", "runtimeStatus"],
  ["Codex", "codexStatus"],
  ["Codex version", "codexVersion"],
  ["Profile", "profileStatus"],
  ["Repository", "repositoryStatus"],
];

function renderRows(element, rows, status) {
  element.replaceChildren(
    ...rows.filter(([, field]) => status[field]).map(([label, field]) => {
      const row = document.createElement("div");
      const term = document.createElement("dt");
      const detail = document.createElement("dd");

      term.textContent = label;
      detail.textContent = status[field];
      detail.dataset.status = status[field];
      row.append(term, detail);
      return row;
    }),
  );
}

function errorMessage(error) {
  const messages = {
    codex_not_found: "Codex executable not found",
    unsupported_codex_version: "Unsupported Codex version",
    codex_schema_incompatible: "Codex schema is incompatible",
    codex_start_failed: "Codex failed to start",
    codex_connection_failed: "Codex connection failed",
  };
  return messages[error] ?? "Codex connection failed";
}

async function loadStatus() {
  const error = document.querySelector("#backend-error");

  try {
    const status = await window.__TAURI_INTERNALS__.invoke("app_status");
    renderRows(document.querySelector("#application-status"), applicationRows, status);
    renderRows(document.querySelector("#runtime-status"), runtimeRows, status);
    const button = document.querySelector("#codex-connection");
    const connectionError = document.querySelector("#connection-error");
    button.disabled = status.codexStatus === "connecting" || status.codexStatus === "disconnecting";
    button.textContent = status.codexStatus === "connected" ? "Disconnect Codex" : "Connect Codex";
    if (status.codexError) {
      connectionError.textContent = errorMessage(status.codexError);
      connectionError.hidden = false;
    } else {
      connectionError.hidden = true;
    }
  } catch (_error) {
    error.hidden = false;
  }
}

async function toggleCodexConnection() {
  const button = document.querySelector("#codex-connection");
  const error = document.querySelector("#connection-error");
  button.disabled = true;
  error.hidden = true;
  try {
    const status = await window.__TAURI_INTERNALS__.invoke("app_status");
    await window.__TAURI_INTERNALS__.invoke(
      status.codexStatus === "connected" ? "disconnect_codex" : "connect_codex",
    );
  } catch (connectionError) {
    error.textContent = errorMessage(connectionError);
    error.hidden = false;
  } finally {
    await loadStatus();
  }
}

document.querySelector("#codex-connection").addEventListener("click", () => {
  void toggleCodexConnection();
});

void loadStatus();
