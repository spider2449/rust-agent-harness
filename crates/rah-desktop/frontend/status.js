const applicationRows = [
  ["Version", "appVersion"],
  ["Platform", "platform"],
  ["Desktop shell", "desktopShell"],
];

const runtimeRows = [
  ["RAH Runtime", "runtimeStatus"],
  ["Codex", "codexStatus"],
  ["Codex source", "codexSource"],
  ["Codex version", "codexVersion"],
  ["Profile", "profileStatus"],
  ["Repository", "repositoryStatus"],
  ["Repository tools", "repositoryToolsStatus"],
  ["Model configuration", "modelConfigurationStatus"],
];

let chatRunning = false;
let activeAssistant = null;
let resumeAvailable = false;
let resumeUsed = false;
let renderedModelConfiguration = null;
let renderedCommitReview = null;
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
      detail.textContent = field === "codexSource" ? ({
        override: "Explicit override",
        certified_baseline: "Certified baseline",
        path: "PATH compatibility fallback",
      }[status[field]] ?? status[field]) : status[field];
      detail.dataset.status = status[field];
      row.append(term, detail);
      return row;
    }),
  );
}

function errorMessage(error) {
  const messages = {
    codex_not_found: "Codex executable not found",
    codex_baseline_invalid: "Certified Codex baseline is invalid",
    codex_host_unsupported: "Certified Codex baseline requires Windows x64",
    unsupported_codex_version: "Unsupported Codex version",
    codex_schema_incompatible: "Codex schema is incompatible",
    codex_start_failed: "Codex failed to start",
    codex_connection_failed: "Codex connection failed",
    tool_registry_failed: "Desktop tool registry unavailable",
    chat_empty_prompt: "Enter a message before sending",
    chat_prompt_too_large: "Message is too large",
    codex_not_connected: "Connect Codex to chat",
    chat_not_running: "No chat turn is running",
    chat_already_running: "A chat turn is already running",
    chat_start_failed: "Chat could not start",
    chat_runtime_failed: "Chat failed",
    chat_cancelled: "Chat was cancelled",
    conversation_context_limit: "Conversation context limit reached; start a new conversation context",
    conversation_history_busy: "Conversation history cannot be cleared while chat is running.",
    conversation_history_clear_failed: "Conversation history could not be cleared.",
    conversation_resume_unavailable: "Previous conversation is unavailable.",
    conversation_resume_busy: "Conversation cannot be resumed while chat is running.",
    conversation_resume_reconnect_required: "Reconnect Codex before resuming this conversation.",
    conversation_resume_too_large: "Previous conversation exceeds the replay limit. Start a new conversation context.",
    conversation_resume_persistence_failed: "Previous conversation could not be resumed.",
    conversation_resume_persistence_incompatible: "Previous conversation is unavailable.",
    git_unavailable: "Git is unavailable",
    repository_not_selected: "Choose a repository first",
    repository_invalid: "Selected folder is not a valid repository root",
    repository_observation_failed: "Repository observation failed",
    repository_dialog_failed: "Repository picker failed",
    repository_busy: "Repository selection is unavailable while chat is running",
    repository_action_invalid: "That repository action is no longer available. Refresh and choose it again.",
    repository_action_stale: "Repository changed after it was displayed. Refresh and choose a new action.",
    model_configuration_invalid: "Invalid model configuration",
    model_configuration_busy: "Model configuration is unavailable while chat is running",
    commit_identity_invalid: "Commit identity is invalid",
    commit_identity_save_failed: "Commit identity could not be saved",
    commit_authorization_unavailable: "Commit authorization is unavailable. Refresh and reconnect if needed.",
    commit_authorization_stale: "The staged review is stale. Refresh and review again.",
    commit_authorization_failed: "Commit authorization failed. Refresh and review again.",
    preferences_save_failed: "Model preferences could not be saved.",
  };
  return messages[error] ?? "Desktop frontend unavailable";
}

function modelHint(provider) {
  const hints = {
    inherit: "Uses Codex host configuration",
    openai: "Codex built-in OpenAI provider",
    ollama: "Codex built-in Ollama provider",
    lm_studio: "Codex built-in LM Studio provider",
    llama_cpp: "",
  };
  return hints[provider] ?? "Invalid model configuration";
}

function renderModelConfiguration(configuration) {
  renderedModelConfiguration = configuration;
  const provider = document.querySelector("#model-provider");
  const model = document.querySelector("#model-identifier");
  provider.value = configuration.provider;
  model.value = configuration.model ?? "";
  model.disabled = configuration.provider === "inherit" || chatRunning;
  provider.disabled = chatRunning;
  const llama = configuration.provider === "llama_cpp";
  const endpointControls = document.querySelector("#llama-cpp-endpoint");
  endpointControls.hidden = !llama;
  for (const element of endpointControls.querySelectorAll("select, input")) element.disabled = chatRunning;
  if (configuration.endpoint) {
    document.querySelector("#llama-cpp-scheme").value = configuration.endpoint.scheme;
    document.querySelector("#llama-cpp-host").value = configuration.endpoint.host;
    document.querySelector("#llama-cpp-port").value = configuration.endpoint.port;
  }
  const normalized = document.querySelector("#llama-cpp-normalized-endpoint");
  normalized.textContent = configuration.endpoint?.normalized ?? "";
  normalized.hidden = !configuration.endpoint;
  document.querySelector("#llama-cpp-insecure-warning").hidden = !llama || !configuration.insecureTransport;
  const readiness = {
    not_tested: "Not tested", checking: "Checking…", ready: "Ready", loading: "Model loading",
    unreachable: "Unreachable", tls_failure: "TLS failure", check_failed: "Health check failed",
  };
  document.querySelector("#llama-cpp-readiness").textContent = readiness[configuration.readiness] ?? "Not tested";
  document.querySelector("#test-llama-cpp-endpoint").disabled = !llama || chatRunning || configuration.readiness === "checking";
  document.querySelector("#apply-model-configuration").disabled = chatRunning;
  document.querySelector("#reset-model-preferences").disabled = chatRunning;
  document.querySelector("#model-hint").textContent = modelHint(configuration.provider);
}

async function refreshModelConfiguration(invoke) {
  renderModelConfiguration(await invoke("model_configuration"));
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
      if (file.unstageActionId) {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.repositoryAction = "unstage";
        button.dataset.actionId = file.unstageActionId;
        button.textContent = "Unstage";
        article.append(button);
      }
      return article;
    }) : [emptyEntry("No changes")]));
  };
  status.replaceChildren(...(snapshot.statusEntries.length ? snapshot.statusEntries.map((entry) => {
    const item = document.createElement("article");
    item.textContent = `${entry.indexState}/${entry.worktreeState} ${entry.path}`;
    if (entry.stageActionId) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.repositoryAction = "stage";
      button.dataset.actionId = entry.stageActionId;
      button.textContent = "Stage";
      item.append(" ", button);
    } else if (entry.stagingNote) {
      const note = document.createElement("span");
      note.textContent = ` — ${entry.stagingNote}`;
      item.append(note);
    }
    return item;
  }) : [emptyEntry("Working tree clean")]));
  renderDiff(document.querySelector("#worktree-diff-entries"), snapshot.worktreeDiff);
  renderDiff(document.querySelector("#staged-diff-entries"), snapshot.stagedDiff);
  renderedCommitReview = snapshot.review;
  const reviewState = ({
    no_staged_changes: "No staged changes",
    review_available: "Review available — complete within bounded observation",
    review_binary_unsupported: "Binary staged content — review is not authorizable in v0.12",
    review_unavailable: "Review unavailable",
  }[snapshot.review.state] ?? "Review unavailable");
  const authorization = snapshot.review.authorizationState;
  document.querySelector("#staged-review-state").textContent = authorization === "authorized_pending"
    ? "Authorized for one future commit request"
    : reviewState;
  const authorize = document.querySelector("#authorize-commit");
  authorize.hidden = !snapshot.review.canAuthorize;
  authorize.disabled = !snapshot.review.canAuthorize;
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
  if (payload.commit) {
    const commit = document.createElement("span");
    const commitLabels = {
      invalid_input: "Commit input rejected",
      precondition_failed: "Commit not performed — review again",
      known_no_effect: "Commit had no effect — review again",
      committed_verified: "Commit verified",
      uncertain: "Commit outcome uncertain — inspect manually; do not retry",
    };
    commit.textContent = commitLabels[payload.commit.status] ?? "Commit result unavailable";
    entry.append(commit);
    if (payload.commit.commitOid) {
      const oid = document.createElement("code");
      oid.textContent = payload.commit.commitOid;
      entry.append(oid);
    }
  }
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
  const reconnectRequired = status.repositoryToolsStatus === "reconnect required"
    || status.modelConfigurationStatus === "reconnect required";
  const prompt = document.querySelector("#chat-prompt");
  const send = document.querySelector("#chat-send");
  document.querySelector("#chat-hint").textContent = connected
    ? (reconnectRequired ? "Reconnect Codex to chat" : (chatRunning ? "Chat running" : "Chat ready"))
    : "Connect Codex to chat";
  prompt.disabled = !connected || reconnectRequired || chatRunning;
  send.disabled = !connected || reconnectRequired;
  send.textContent = chatRunning ? "Cancel Turn" : "Send";
  document.querySelector("#new-conversation").disabled = chatRunning;
  document.querySelector("#resume-previous-conversation").disabled = !resumeAvailable
    || !connected
    || chatRunning
    || status.repositoryToolsStatus === "reconnect required"
    || status.modelConfigurationStatus === "reconnect required"
    || resumeUsed;
  document.querySelector("#clear-conversation-history").disabled = chatRunning;
  document.querySelector("#choose-repository").disabled = status.codexStatus === "connecting" || status.codexStatus === "disconnecting" || chatRunning;
  const model = document.querySelector("#model-identifier");
  const provider = document.querySelector("#model-provider");
  const endpointControls = document.querySelector("#llama-cpp-endpoint");
  document.querySelector("#apply-model-configuration").disabled = chatRunning;
  document.querySelector("#reset-model-preferences").disabled = chatRunning;
  provider.disabled = chatRunning;
  model.disabled = chatRunning || provider.value === "inherit";
  for (const element of endpointControls.querySelectorAll("select, input")) {
    element.disabled = chatRunning;
  }
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

function appendContextSeparator(reason) {
  const messages = document.querySelector("#chat-messages");
  const separator = document.createElement("section");
  const title = document.createElement("strong");
  const detail = document.createElement("p");
  const labels = {
    new_conversation: "New conversation context",
    repository_changed: "Repository changed",
    model_configuration_changed: "Model configuration changed",
    repository_and_model_changed: "Repository and model configuration changed",
    application_restarted: "Application restarted",
    history_trimmed: "Earlier conversation history was trimmed",
  };
  separator.className = "conversation-context-separator";
  title.textContent = reason === "history_trimmed" ? "Conversation history" : "New conversation context";
  detail.textContent = labels[reason] ?? "New conversation context";
  separator.append(title, detail);
  messages.append(separator);
  messages.scrollTop = messages.scrollHeight;
}

function showPersistenceWarning(warning) {
  const messages = {
    restore_failed: "Previous conversation could not be restored.",
    save_failed: "Conversation history could not be saved.",
  };
  const error = document.querySelector("#chat-error");
  error.textContent = messages[warning] ?? "Conversation history could not be saved.";
  error.hidden = false;
}

function renderTranscript(transcript) {
  resumeAvailable = transcript.resumeAvailable;
  for (const record of transcript.records) {
    if (record.kind === "completed_message") {
      appendMessage(record.role === "user" ? "You" : "RAH", record.text);
    } else if (record.kind === "context_separator") {
      appendContextSeparator(record.reason);
    }
  }
  if (transcript.warning) showPersistenceWarning(transcript.warning);
}

async function replaceTranscript(invoke) {
  // A repository namespace is a presentation boundary, not a separator in a
  // shared transcript. Remove every rendered record before displaying only the
  // newly selected namespace.
  document.querySelector("#chat-messages").replaceChildren();
  activeAssistant = null;
  resumeUsed = false;
  renderTranscript(await invoke("conversation_transcript"));
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
  let disconnected = false;
  try {
    const status = await invoke("app_status");
    disconnected = status.codexStatus === "connected";
    await invoke(disconnected ? "disconnect_codex" : "connect_codex");
  } catch (connectionError) {
    error.textContent = errorMessage(connectionError);
    error.hidden = false;
  } finally {
    try {
      await loadStatus(invoke);
      if (disconnected) await refreshRepository(invoke);
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
  await listen("conversation_persistence_warning", (event) => showPersistenceWarning(event.payload));
  await listen("desktop_preferences_warning", (event) => {
    const error = document.querySelector("#model-error");
    error.textContent = event.payload === "preferences_restore_failed" ? "Model preferences could not be restored." : "Model preferences could not be saved.";
    error.hidden = false;
  });
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
      await replaceTranscript(invoke);
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
  document.querySelector("#new-conversation").addEventListener("click", async () => {
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      await invoke("new_conversation");
      resumeUsed = true;
      appendContextSeparator();
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  document.querySelector("#clear-conversation-history").addEventListener("click", async () => {
    document.querySelector("#clear-history-confirmation").showModal();
  });
  document.querySelector("#clear-history-confirmation").addEventListener("close", async (event) => {
    if (event.target.returnValue !== "confirm") return;
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      await invoke("clear_conversation_history");
      resumeAvailable = false;
      resumeUsed = true;
      document.querySelector("#chat-messages").replaceChildren();
      appendMessage("RAH", "Conversation history cleared");
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  document.querySelector("#save-commit-identity").addEventListener("click", async () => {
    try {
      await invoke("set_commit_identity", {
        name: document.querySelector("#commit-identity-name").value,
        email: document.querySelector("#commit-identity-email").value,
      });
      const identity = await invoke("commit_identity");
      document.querySelector("#commit-identity-state").textContent = identity.configured ? "Configured" : "Not configured";
      await refreshRepository(invoke);
      await loadStatus(invoke);
    } catch (error) {
      document.querySelector("#repository-error").textContent = errorMessage(error);
      document.querySelector("#repository-error").hidden = false;
    }
  });
  document.querySelector("#authorize-commit").addEventListener("click", async () => {
    if (!renderedCommitReview?.reviewId) return;
    try {
      const result = await invoke("repository_authorize_commit_review", { reviewId: renderedCommitReview.reviewId });
      document.querySelector("#staged-review-state").textContent = result.authorizationState === "authorized_pending"
        ? "Authorized for one future commit request" : "Review authorization unavailable";
      document.querySelector("#authorize-commit").hidden = true;
    } catch (error) {
      document.querySelector("#repository-error").textContent = errorMessage(error);
      document.querySelector("#repository-error").hidden = false;
    }
  });
  document.querySelector(".repository").addEventListener("click", async (event) => {
    const button = event.target.closest("button[data-repository-action]");
    if (!button) return;
    const error = document.querySelector("#repository-error");
    error.hidden = true;
    button.disabled = true;
    try {
      await invoke(button.dataset.repositoryAction === "stage" ? "repository_stage_action" : "repository_unstage_action", { actionId: button.dataset.actionId });
      await refreshRepository(invoke);
    } catch (repositoryError) {
      error.textContent = errorMessage(repositoryError);
      error.hidden = false;
      await refreshRepository(invoke);
    }
  });
  document.querySelector("#resume-previous-conversation").addEventListener("click", async () => {
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      await invoke("resume_previous_conversation");
      resumeUsed = true;
      const success = document.querySelector("#resume-success");
      success.textContent = "Previous conversation resumed in current context.";
      success.hidden = false;
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  document.querySelector("#model-provider").addEventListener("change", () => {
    const provider = document.querySelector("#model-provider");
    const model = document.querySelector("#model-identifier");
    model.disabled = chatRunning || provider.value === "inherit";
    document.querySelector("#llama-cpp-endpoint").hidden = provider.value !== "llama_cpp";
    document.querySelector("#model-hint").textContent = modelHint(provider.value);
  });
  document.querySelector("#apply-model-configuration").addEventListener("click", async () => {
    const error = document.querySelector("#model-error");
    error.hidden = true;
    const provider = document.querySelector("#model-provider").value;
    const model = document.querySelector("#model-identifier").value;
    const llamaCppEndpoint = provider === "llama_cpp" ? {
      scheme: document.querySelector("#llama-cpp-scheme").value,
      host: document.querySelector("#llama-cpp-host").value,
      port: Number(document.querySelector("#llama-cpp-port").value),
    } : null;
    try {
      await invoke("set_model_configuration", {
        provider,
        model: provider === "inherit" ? null : model,
        llamaCppEndpoint,
      });
      await refreshModelConfiguration(invoke);
      await loadStatus(invoke);
      if ((await invoke("model_configuration")).status === "reconnect required") {
        error.textContent = "Reconnect Codex to activate this model configuration.";
        error.hidden = false;
      }
    } catch (modelError) {
      error.textContent = errorMessage(modelError);
      error.hidden = false;
    }
  });
  document.querySelector("#reset-model-preferences").addEventListener("click", async () => {
    const error = document.querySelector("#model-error");
    error.hidden = true;
    try {
      await invoke("reset_model_preferences");
      await refreshModelConfiguration(invoke);
      await loadStatus(invoke);
    } catch (resetError) {
      error.textContent = errorMessage(resetError);
      error.hidden = false;
    }
  });
  document.querySelector("#test-llama-cpp-endpoint").addEventListener("click", async () => {
    const error = document.querySelector("#model-error");
    error.hidden = true;
    try {
      const readinessRequest = invoke("test_llama_cpp_endpoint");
      if (renderedModelConfiguration) {
        renderModelConfiguration({ ...renderedModelConfiguration, readiness: "checking" });
      }
      await readinessRequest;
      const refreshReadiness = async () => {
        const configuration = await invoke("model_configuration");
        renderModelConfiguration(configuration);
        if (configuration.readiness === "checking") {
          window.setTimeout(() => { void refreshReadiness(); }, 100);
        }
      };
      void refreshReadiness();
    } catch (readinessError) {
      if (renderedModelConfiguration) {
        renderModelConfiguration({ ...renderedModelConfiguration, readiness: "check_failed" });
      }
      error.textContent = "Health check failed";
      error.hidden = false;
    }
  });
  document.querySelector("#chat-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    if (chatRunning) {
      try {
        await invoke("cancel_chat");
      } catch (error) {
        showChatError(error);
      }
      return;
    }
    const prompt = document.querySelector("#chat-prompt");
    const chatError = document.querySelector("#chat-error");
    chatError.hidden = true;
    try {
      const result = await invoke("send_chat", { prompt: prompt.value });
      if (result.contextChange) appendContextSeparator(result.contextChange);
      appendMessage("You", prompt.value);
      prompt.value = "";
      chatRunning = true;
      await loadStatus(invoke);
    } catch (error) {
      showChatError(error);
    }
  });
  await loadStatus(invoke);
  await replaceTranscript(invoke);
  await refreshModelConfiguration(invoke);
  const identity = await invoke("commit_identity");
  document.querySelector("#commit-identity-state").textContent = identity.configured ? "Configured" : "Not configured";
  const preferencesWarning = await invoke("desktop_preferences_warning");
  if (preferencesWarning) {
    const error = document.querySelector("#model-error");
    error.textContent = preferencesWarning === "preferences_restore_failed" ? "Model preferences could not be restored." : "Model preferences could not be saved.";
    error.hidden = false;
  }
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
