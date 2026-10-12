// RAH Web UI. Model and tool output is untrusted: it is only ever inserted
// as text (textContent), never as HTML.
"use strict";

const $ = (id) => document.getElementById(id);

const state = {
  token: null,
  socket: null,
  sessions: [],
  current: null,
  running: false,
  liveText: "",
  retry: 500,
};

// The token arrives in the URL fragment, which browsers never send to a
// server. Keep it for this tab only and remove it from the address bar.
function readToken() {
  const match = /token=([0-9a-f]+)/.exec(location.hash);
  if (match) {
    sessionStorage.setItem("rah-token", match[1]);
    history.replaceState(null, "", location.pathname);
  }
  return sessionStorage.getItem("rah-token");
}

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function showNotice(text) {
  $("notice").textContent = text;
  $("notice").hidden = !text;
}

async function api(path, options = {}) {
  const response = await fetch(`/api${path}`, {
    ...options,
    headers: { Authorization: `Bearer ${state.token}`, "Content-Type": "application/json" },
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(body.error || `${response.status} ${response.statusText}`);
  return body;
}

function renderContent(output) {
  return (output.content || [])
    .map((item) => (item.type === "text" ? item.value : JSON.stringify(item.value, null, 2)))
    .join("\n");
}

// ---- Sessions -------------------------------------------------------------

async function loadSessions() {
  state.sessions = await api("/sessions");
  state.sessions.sort((a, b) => b.created - a.created);
  const list = $("sessions");
  list.replaceChildren();
  for (const info of state.sessions) {
    const item = el("li", info.id === state.current ? "current" : "", info.id.slice(0, 8));
    const when = new Date(info.created * 1000).toLocaleString();
    item.append(el("small", "", info.parent ? `fork of ${info.parent.session.slice(0, 8)} @${info.parent.at} · ${when}` : when));
    item.title = info.id;
    item.addEventListener("click", () => openSession(info.id));
    list.append(item);
  }
}

async function openSession(id) {
  state.current = id;
  setRunning(false);
  await loadSessions();
  await loadTranscript();
  $("input").disabled = false;
  $("send").disabled = false;
  $("input").focus();
}

async function newSession() {
  const info = await api("/sessions", { method: "POST" });
  await openSession(info.id);
}

async function fork(at) {
  const info = await api(`/sessions/${state.current}/fork`, { method: "POST", body: JSON.stringify({ at }) });
  await openSession(info.id);
}

function entryNode(record) {
  const entry = record.entry;
  const node = el("li", "entry");
  const meta = el("div", "meta");
  const body = el("pre");
  switch (entry.kind) {
    case "turn_started":
      node.classList.add("user");
      meta.append(el("span", "", "you"));
      body.textContent = entry.input;
      break;
    case "assistant_message":
      meta.append(el("span", "", "assistant"));
      body.textContent = entry.text;
      break;
    case "tool_call":
      node.classList.add("tool");
      meta.append(el("span", "", `tool call · ${entry.name}`));
      body.textContent = JSON.stringify(entry.arguments, null, 2);
      break;
    case "tool_result": {
      node.classList.add("tool");
      meta.append(el("span", "", entry.output.is_error ? "tool error" : "tool result"));
      meta.append(el("span", `effect-${entry.effect}`, `effect: ${entry.effect}`));
      body.textContent = renderContent(entry.output);
      break;
    }
    case "turn_completed":
      return null;
    case "turn_failed":
      node.classList.add("failed");
      meta.append(el("span", "", "turn failed"));
      body.textContent = entry.message;
      break;
    default:
      meta.append(el("span", "", entry.kind));
  }
  meta.append(el("span", "spacer"));
  meta.append(el("span", "", `#${record.seq}`));
  const forkButton = el("button", "ghost fork", "fork here");
  forkButton.type = "button";
  forkButton.title = `Start a new session continuing after record ${record.seq}`;
  forkButton.addEventListener("click", () => fork(record.seq).catch((error) => showNotice(error.message)));
  meta.append(forkButton);
  node.append(meta, body);
  return node;
}

async function loadTranscript() {
  if (!state.current) return;
  const records = await api(`/sessions/${state.current}`);
  const list = $("transcript");
  list.replaceChildren(...records.map(entryNode).filter(Boolean));
  list.scrollTop = list.scrollHeight;
}

// ---- Turns ----------------------------------------------------------------

function setRunning(running) {
  state.running = running;
  $("send").disabled = running || !state.current;
  $("cancel").hidden = !running;
  if (!running) {
    state.liveText = "";
    $("live").hidden = true;
  }
}

function showLive(text) {
  state.liveText += text;
  $("live").textContent = state.liveText;
  $("live").hidden = false;
}

const NOT_CONNECTED = "Not connected to RAH.";

function send(message) {
  if (state.socket && state.socket.readyState === WebSocket.OPEN) {
    state.socket.send(JSON.stringify(message));
    return true;
  }
  showNotice(NOT_CONNECTED);
  return false;
}

function submit(event) {
  event.preventDefault();
  const input = $("input").value.trim();
  if (!input || !state.current || state.running) return;
  if (send({ type: "turn", session: state.current, input })) {
    $("input").value = "";
    setRunning(true);
    showLive(`you: ${input}\n`);
  }
}

function onEvent(session, event) {
  if (session !== state.current) return;
  switch (event.type) {
    case "model_delta":
      showLive(event.delta);
      break;
    case "model_reasoning_started":
      showLive("[thinking]\n");
      break;
    case "tool_requested":
      showLive(`\n[tool] ${event.tool_call.name} ${JSON.stringify(event.tool_call.input)}\n`);
      break;
    case "tool_finished":
      showLive(`[${event.output.is_error ? "tool error" : "tool result"}] ${renderContent(event.output)}\n`);
      break;
    case "completed":
    case "failed":
    case "cancelled":
      if (event.type === "failed") showNotice(`Turn failed: ${event.message}`);
      setRunning(false);
      loadTranscript().catch((error) => showNotice(error.message));
      break;
    default:
      break;
  }
}

// ---- Approvals ------------------------------------------------------------

function showApproval(request) {
  const card = el("div", "approval");
  card.id = `approval-${request.id}`;
  card.append(el("strong", "", `Allow ${request.tool}?`));
  // The reason names the permission the policy requires; the tool's own
  // declared permission can be lower and would only mislead here.
  card.append(el("div", "", request.reason));
  card.append(el("pre", "", JSON.stringify(request.input, null, 2)));
  const buttons = el("div", "buttons");
  for (const [label, approve] of [["Deny", false], ["Allow", true]]) {
    const button = el("button", approve ? "" : "ghost", label);
    button.type = "button";
    button.addEventListener("click", () => {
      send({ type: "approval", id: request.id, approve });
      card.remove();
    });
    buttons.append(button);
  }
  card.append(buttons);
  $("approvals").append(card);
}

// ---- Connection -----------------------------------------------------------

function connect() {
  const socket = new WebSocket(`ws://${location.host}/api/ws?token=${state.token}`);
  state.socket = socket;
  socket.addEventListener("open", () => {
    state.retry = 500;
    $("connection").textContent = "connected";
    $("connection").className = "badge good";
    // Keep other notices, such as a failed startup request.
    if ($("notice").textContent === NOT_CONNECTED) showNotice("");
  });
  socket.addEventListener("message", (message) => {
    const data = JSON.parse(message.data);
    if (data.type === "event") onEvent(data.session, data.event);
    else if (data.type === "approval_required") showApproval(data);
    else if (data.type === "approval_resolved") $(`approval-${data.id}`)?.remove();
    else if (data.type === "error") {
      showNotice(data.message);
      setRunning(false);
    }
  });
  socket.addEventListener("close", () => {
    $("connection").textContent = "disconnected";
    $("connection").className = "badge bad";
    setRunning(false);
    setTimeout(connect, state.retry);
    state.retry = Math.min(state.retry * 2, 10000);
  });
}

// ---- Plugins --------------------------------------------------------------

// ---- Plugin management ----------------------------------------------------

const editor = { mode: null, target: null };

function report(error) {
  showNotice(error.message);
}

async function manage(path, method, body) {
  const options = { method };
  if (body !== undefined) options.body = JSON.stringify(body);
  const result = await api(path, options);
  if (result.state && result.state !== "active" && result.state !== "unmounted") {
    showNotice(`${result.id}: ${result.state}${result.detail ? ` (${result.detail})` : ""}`);
  } else {
    showNotice("");
  }
  await loadPlugins();
  return result;
}

function openEditor(mode, plugin) {
  editor.mode = mode;
  editor.target = plugin;
  const titles = { add: "Add plugin", configure: `Configure ${plugin?.id}`, replace: `Replace ${plugin?.id}` };
  $("editor-title").textContent = titles[mode];
  $("editor-kind-row").hidden = mode === "configure";
  $("editor-id-row").hidden = mode === "configure";
  $("editor-kind").value = plugin?.kind || $("editor-kind").options[0]?.value || "";
  $("editor-id").value = "";
  const config = plugin && plugin.config !== undefined ? plugin.config : {};
  $("editor-config").value = JSON.stringify(config ?? {}, null, 2);
  $("editor").hidden = false;
  $("editor-config").focus();
}

async function saveEditor(event) {
  event.preventDefault();
  let config;
  try {
    config = JSON.parse($("editor-config").value || "null");
  } catch (error) {
    showNotice(`The configuration is not valid JSON: ${error.message}`);
    return;
  }
  const request = { kind: $("editor-kind").value, id: $("editor-id").value.trim() || null, config };
  const target = editor.target ? encodeURIComponent(editor.target.id) : "";
  try {
    if (editor.mode === "add") await manage("/plugins", "POST", request);
    else if (editor.mode === "configure") await manage(`/plugins/${target}/config`, "PUT", { config });
    else await manage(`/plugins/${target}/replace`, "POST", request);
    $("editor").hidden = true;
  } catch (error) {
    report(error);
  }
}

function removePlugin(plugin) {
  let warning = `Unmount ${plugin.id}?`;
  if (plugin.kind === "policy-authority") warning += "\n\nWithout a policy plugin, model tool calls run UNGUARDED.";
  if (plugin.kind === "frontend-web") warning += "\n\nThis is the Web UI you are using; it will stop responding.";
  if (!window.confirm(warning)) return;
  manage(`/plugins/${encodeURIComponent(plugin.id)}`, "DELETE").catch(report);
}

function pluginRow(plugin) {
  const row = el("tr");
  const name = el("td", "", plugin.id);
  if (plugin.kind && plugin.kind !== plugin.id) name.append(el("span", "kind", plugin.kind));
  const cell = el("td", `state-${plugin.state}`, plugin.state);
  if (plugin.detail) cell.title = plugin.detail;
  if (plugin.kind && !plugin.locked) {
    const actions = el("div", "row-actions");
    const action = (label, handler) => {
      const button = el("button", "ghost", label);
      button.type = "button";
      button.addEventListener("click", handler);
      actions.append(button);
    };
    action("Restart", () => manage(`/plugins/${encodeURIComponent(plugin.id)}/restart`, "POST").catch(report));
    action("Configure", () => openEditor("configure", plugin));
    action("Replace", () => openEditor("replace", plugin));
    action("Remove", () => removePlugin(plugin));
    name.append(actions);
  }
  row.append(name, cell);
  return row;
}

async function loadCatalog() {
  try {
    const catalog = await api("/catalog");
    $("editor-kind").replaceChildren(...catalog.kinds.map((kind) => {
      const option = el("option", "", kind);
      option.value = kind;
      return option;
    }));
    $("add-plugin").hidden = false;
  } catch {
    $("add-plugin").hidden = true;
  }
}

async function loadPlugins() {
  const [plugins, info] = await Promise.all([api("/plugins"), api("/info")]);
  $("plugins").replaceChildren(...plugins.map(pluginRow));
  $("tools").replaceChildren(...info.tools.map((tool) => el("li", "", `${tool.name} (${tool.permission})`)));
  const guard = $("guard");
  guard.hidden = false;
  guard.textContent = info.guarded ? "guarded" : "UNGUARDED";
  guard.className = `badge ${info.guarded ? "good" : "bad"}`;
}

async function start() {
  state.token = readToken();
  if (!state.token) {
    showNotice("Open the URL that rah printed at startup; it carries the access token.");
    return;
  }
  $("composer").addEventListener("submit", submit);
  $("input").addEventListener("keydown", (event) => {
    if (event.key === "Enter" && !event.shiftKey) submit(event);
  });
  $("cancel").addEventListener("click", () => send({ type: "cancel" }));
  $("new-session").addEventListener("click", () => newSession().catch((error) => showNotice(error.message)));
  $("toggle-plugins").addEventListener("click", () => {
    $("plugins-panel").hidden = !$("plugins-panel").hidden;
  });
  $("refresh-plugins").addEventListener("click", () => loadPlugins().catch(report));
  $("add-plugin").addEventListener("click", () => openEditor("add", null));
  $("editor").addEventListener("submit", saveEditor);
  $("editor-cancel").addEventListener("click", () => {
    $("editor").hidden = true;
  });
  try {
    await loadCatalog();
    await loadPlugins();
    await loadSessions();
    if (state.sessions.length > 0) await openSession(state.sessions[0].id);
  } catch (error) {
    showNotice(error.message);
  }
  connect();
}

start();
