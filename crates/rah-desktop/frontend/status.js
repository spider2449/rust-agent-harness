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
  ["Repository tools", "repositoryToolsStatus"],
];

let chatRunning = false;
let activeAssistant = null;
const maxActivityEntries = 100;

const tauriApiRetryDelayMs = 100;
const tauriApiRetryAttempts = 20;

function setFrontendBootStatus(message) {
  document.querySelector("#frontend-boot-status").textContent = message;
}

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
    tool_registry_failed: "Desktop tool registry unavailable",
    chat_empty_prompt: "Enter a message before sending",
    chat_prompt_too_large: "Message is too large",
    codex_not_connected: "Connect Codex to chat",
    chat_already_running: "A chat turn is already running",
    chat_start_failed: "Chat could not start",
    chat_runtime_failed: "Chat failed",
    chat_cancelled: "Chat was cancelled",
    git_unavailable: "Git is unavailable",
    repository_not_selected: "Choose a repository first",
    repository_invalid: "Selected folder is not a valid repository root",
    repository_observation_failed: "Repository observation failed",
    repository_dialog_failed: "Repository picker failed",
    repository_busy: "Repository selection is unavailable while chat is running",
  };
  return messages[error] ?? "Desktop frontend unavailable";
}

function renderRepositorySnapshot(snapshot) {
  document.querySelector("#repository-path").textContent = snapshot.path;
  const status = document.querySelector("#repository-status-entries");
  const renderDiff = (element, files) => {
    element.replaceChildren(...(files.length ? files.map((file) => {
      const article = document.createElement("article");
      const title = document.createElement("strong");
      const patch = document.createElement("pre");
      title.textContent = `${file.changeKind} ${file.newPath ?? file.oldPath ?? "[unknown path]"} +${file.addedLines ?? 0} -${file.deletedLines ?? 0}`;
      patch.textContent = file.patch ?? (file.binary ? "Binary file changed" : "No patch");
      article.append(title, patch);
      return article;
    }) : [emptyEntry("No changes")]));
  };
  status.replaceChildren(...(snapshot.statusEntries.length ? snapshot.statusEntries.map((entry) => {
    const item = document.createElement("article");
    item.textContent = `${entry.indexState}/${entry.worktreeState} ${entry.path}`;
    return item;
  }) : [emptyEntry("Working tree clean")]));
  renderDiff(document.querySelector("#worktree-diff-entries"), snapshot.worktreeDiff);
  renderDiff(document.querySelector("#staged-diff-entries"), snapshot.stagedDiff);
}

function emptyEntry(text) {
  const item = document.createElement("p");
  item.textContent = text;
  return item;
}

async function refreshRepository(invoke) {
  const error = document.querySelector("#repository-error");
  error.hidden = true;
  try {
    renderRepositorySnapshot(await invoke("repository_snapshot"));
  } catch (repositoryError) {
    error.textContent = errorMessage(repositoryError);
    error.hidden = false;
  }
}

function appendActivity(payload) {
  const entries = document.querySelector("#activity-entries");
  const entry = document.createElement("article");
  const tool = document.createElement("strong");
  const state = document.createElement("span");
  const labels = {
    tool_requested: "Requested",
    tool_started: "Running",
    tool_finished: payload.result === "failed" ? "Failed" : "Completed",
  };

  entry.className = "activity-entry";
  entry.dataset.state = payload.kind === "tool_finished" ? payload.result : payload.kind;
  tool.textContent = payload.tool;
  state.textContent = labels[payload.kind] ?? "Unknown";
  entry.append(tool, state);
  entries.append(entry);
  while (entries.children.length > maxActivityEntries) {
    entries.firstElementChild.remove();
  }
  entries.scrollTop = entries.scrollHeight;
}

function showBackendError() {
  const error = document.querySelector("#backend-error");
  error.textContent = "Desktop backend unavailable";
  error.hidden = false;
  document.querySelector("#codex-connection").disabled = true;
}

function waitForTauriApi() {
  return new Promise((resolve) => {
    let attempts = 0;
    const check = () => {
      const tauri = window.__TAURI__;
      if (tauri?.core?.invoke && tauri?.event?.listen) {
        resolve(tauri);
        return;
      }
      attempts += 1;
      if (attempts >= tauriApiRetryAttempts) {
        resolve(null);
        return;
      }
      window.setTimeout(check, tauriApiRetryDelayMs);
    };
    check();
  });
}

async function loadStatus(invoke) {
  const status = await invoke("app_status");
  renderRows(document.querySelector("#application-status"), applicationRows, status);
  renderRows(document.querySelector("#runtime-status"), runtimeRows, status);
  const button = document.querySelector("#codex-connection");
  const connectionError = document.querySelector("#connection-error");
  button.disabled = status.codexStatus === "connecting" || status.codexStatus === "disconnecting" || chatRunning;
  button.textContent = status.codexStatus === "connected" ? "Disconnect Codex" : "Connect Codex";
  const connected = status.codexStatus === "connected";
  const prompt = document.querySelector("#chat-prompt");
  const send = document.querySelector("#chat-send");
  document.querySelector("#chat-hint").textContent = connected ? (chatRunning ? "Chat running" : "Chat ready") : "Connect Codex to chat";
  prompt.disabled = !connected || chatRunning;
  send.disabled = !connected || chatRunning;
  if (status.codexError) {
    connectionError.textContent = errorMessage(status.codexError);
    connectionError.hidden = false;
  } else {
    connectionError.hidden = true;
  }
}

function appendMessage(role, text) {
  const messages = document.querySelector("#chat-messages");
  const message = document.createElement("article");
  const label = document.createElement("strong");
  const content = document.createElement("p");
  message.className = "chat-message";
  label.textContent = role;
  content.textContent = text;
  message.append(label, content);
  messages.append(message);
  messages.scrollTop = messages.scrollHeight;
  return content;
}

function showChatError(code) {
  const error = document.querySelector("#chat-error");
  error.textContent = errorMessage(code);
  error.hidden = false;
}

function handleChatEvent(invoke, event) {
  const payload = event.payload;
  if (payload.kind === "started") {
    activeAssistant = appendMessage("RAH", "");
  } else if (payload.kind === "delta" && activeAssistant) {
    activeAssistant.textContent += payload.text;
  } else if (payload.kind === "failed" || payload.kind === "cancelled") {
    showChatError(payload.code);
  }
  if (["completed", "failed", "cancelled"].includes(payload.kind)) {
    chatRunning = false;
    activeAssistant = null;
    void loadStatus(invoke).catch(() => showBackendError());
  }
}

async function toggleCodexConnection(invoke) {
  const button = document.querySelector("#codex-connection");
  const error = document.querySelector("#connection-error");
  button.disabled = true;
  error.hidden = true;
  try {
    const status = await invoke("app_status");
    await invoke(status.codexStatus === "connected" ? "disconnect_codex" : "connect_codex");
  } catch (connectionError) {
    error.textContent = errorMessage(connectionError);
    error.hidden = false;
  } finally {
    try {
      await loadStatus(invoke);
    } catch (statusError) {
      console.error("failed to refresh desktop status", statusError);
      showBackendError();
    }
  }
}

async function initializeDesktop() {
  const tauri = await waitForTauriApi();
  if (!tauri) {
    throw new Error("supported Tauri global API is unavailable");
  }
  const { invoke } = tauri.core;
  const { listen } = tauri.event;

  await listen("chat_event", (event) => handleChatEvent(invoke, event));
  await listen("activity_event", (event) => appendActivity(event.payload));
  await listen("repository_snapshot_refresh", () => {
    void refreshRepository(invoke);
  });
  document.querySelector("#codex-connection").addEventListener("click", () => {
    void toggleCodexConnection(invoke);
  });
  document.querySelector("#choose-repository").addEventListener("click", async () => {
    const error = document.querySelector("#repository-error");
    error.hidden = true;
    try {
      await invoke("choose_repository");
      await loadStatus(invoke);
      await refreshRepository(invoke);
    } catch (repositoryError) {
      error.textContent = errorMessage(repositoryError);
      error.hidden = false;
    }
  });
  document.querySelector("#refresh-repository").addEventListener("click", () => {
    void refreshRepository(invoke);
  });
  document.querySelector("#chat-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    if (chatRunning) return;
    const prompt = document.querySelector("#chat-prompt");
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      await invoke("send_chat", { prompt: prompt.value });
      appendMessage("You", prompt.value);
      prompt.value = "";
      chatRunning = true;
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  await loadStatus(invoke);
  setFrontendBootStatus("Desktop UI ready");
}

function startDesktop() {
  void initializeDesktop().catch((error) => {
    console.error("failed to initialize desktop frontend", error);
    setFrontendBootStatus(
      error.message === "supported Tauri global API is unavailable"
        ? "Desktop frontend unavailable"
        : "Desktop backend unavailable",
    );
    showBackendError();
  });
}

if (document.readyState === "complete") {
  startDesktop();
} else {
  window.addEventListener("load", startDesktop, { once: true });
}
