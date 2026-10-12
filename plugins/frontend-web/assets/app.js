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

// Session titles come from the model or the user: untrusted, text only.
function sessionName(info) {
  return info.title || info.preview || "New session";
}

function dayGroup(seconds) {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const day = new Date(seconds * 1000);
  day.setHours(0, 0, 0, 0);
  const days = Math.round((today - day) / 86400000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return "Previous 7 days";
  return "Older";
}

function timeLabel(seconds) {
  const when = new Date(seconds * 1000);
  return dayGroup(seconds) === "Today"
    ? when.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : when.toLocaleDateString();
}

function currentInfo() {
  return state.sessions.find((info) => info.id === state.current);
}

async function loadSessions() {
  state.sessions = await api("/sessions");
  state.sessions.sort((a, b) => (b.updated || b.created) - (a.updated || a.created));
  renderSessions();
  renderHeader();
}

function renderSessions() {
  const query = $("session-search").value.trim().toLowerCase();
  const list = $("sessions");
  list.replaceChildren();
  let group = null;
  for (const info of state.sessions) {
    const haystack = `${info.title || ""} ${info.preview || ""} ${info.id}`.toLowerCase();
    if (query && !haystack.includes(query)) continue;
    const label = dayGroup(info.updated || info.created);
    if (label !== group) {
      group = label;
      list.append(el("li", "group", label));
    }
    list.append(sessionItem(info));
  }
  if (!list.children.length) {
    list.append(el("li", "group", query ? "No matching sessions" : "No sessions yet"));
  }
}

function sessionItem(info) {
  const item = el("li", info.id === state.current ? "session current" : "session");
  item.title = info.id;
  item.dataset.id = info.id;
  const name = el("span", "name", sessionName(info));
  const meta = [timeLabel(info.updated || info.created)];
  if (info.parent) meta.push(`fork of ${info.parent.session.slice(0, 8)}`);
  const actions = el("span", "item-actions");
  for (const [label, text, handler] of [
    ["Rename", "✎", () => editTitle(name, info)],
    ["Delete", "✕", () => deleteSession(info)],
  ]) {
    const button = el("button", "ghost icon", text);
    button.type = "button";
    button.title = label;
    button.setAttribute("aria-label", `${label} session`);
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      handler();
    });
    actions.append(button);
  }
  item.append(name, actions, el("small", "", meta.join(" · ")));
  item.addEventListener("click", () => openSession(info.id).catch(report));
  return item;
}

function renderHeader() {
  const info = currentInfo();
  $("session-head").hidden = !info;
  if (!info) return;
  $("session-title").textContent = sessionName(info);
  $("session-title").classList.toggle("untitled", !info.title);
}

// Edits a title in place: Enter saves, Escape or leaving the field cancels.
function editTitle(host, info) {
  const input = el("input", "title-input");
  input.value = info.title || "";
  input.placeholder = sessionName(info);
  input.maxLength = 120;
  input.setAttribute("aria-label", "Session title");
  let done = false;
  const finish = async (save) => {
    if (done) return;
    done = true;
    if (save) {
      const title = input.value.trim();
      try {
        await api(`/sessions/${info.id}/title`, { method: "PUT", body: JSON.stringify({ title: title || null }) });
      } catch (error) {
        report(error);
      }
    }
    await loadSessions().catch(report);
  };
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") finish(true);
    else if (event.key === "Escape") finish(false);
  });
  input.addEventListener("click", (event) => event.stopPropagation());
  input.addEventListener("blur", () => finish(false));
  host.replaceChildren(input);
  input.focus();
  input.select();
}

async function deleteSession(info) {
  if (!window.confirm(`Delete "${sessionName(info)}"?\n\nThe log is moved to the sessions' deleted folder.`)) return;
  try {
    await api(`/sessions/${info.id}`, { method: "DELETE" });
  } catch (error) {
    report(error);
    return;
  }
  if (info.id === state.current) {
    state.current = null;
    $("transcript").replaceChildren();
    $("input").disabled = true;
    $("send").disabled = true;
  }
  await loadSessions();
}

// Downloads go through fetch, since the API needs the token header.
async function exportSession(format) {
  const response = await fetch(`/api/sessions/${state.current}/export?format=${format}`, {
    headers: { Authorization: `Bearer ${state.token}` },
  });
  if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  const name = /filename="([^"]+)"/.exec(response.headers.get("Content-Disposition") || "");
  const url = URL.createObjectURL(await response.blob());
  const link = el("a");
  link.href = url;
  link.download = name ? name[1] : `rah-session.${format}`;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// A generated title arrives after the turn; poll briefly until it does.
function awaitTitle() {
  clearTimeout(state.titleTimer);
  let tries = 0;
  const check = async () => {
    const info = currentInfo();
    if (!info || info.title || tries++ >= 15) return;
    await loadSessions().catch(() => {});
    state.titleTimer = setTimeout(check, 2000);
  };
  state.titleTimer = setTimeout(check, 1000);
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
  return info;
}

// Sends `input` again in a fork that ends before the turn at `seq`. The
// original session is kept; the fork takes over its title.
async function resend(seq, input) {
  if (state.running) return;
  const title = currentInfo()?.title;
  const info = await api(`/sessions/${state.current}/fork`, { method: "POST", body: JSON.stringify({ at: seq - 1 }) });
  if (title) {
    await api(`/sessions/${info.id}/title`, { method: "PUT", body: JSON.stringify({ title }) }).catch(() => {});
  }
  await openSession(info.id);
  startTurn(input);
}

function editMessage(node, seq, input) {
  const body = node.querySelector("pre");
  const editor = el("div", "edit");
  const area = el("textarea");
  area.value = input;
  area.rows = Math.min(10, input.split("\n").length + 1);
  const buttons = el("div", "buttons");
  const cancel = el("button", "ghost", "Cancel");
  cancel.type = "button";
  cancel.addEventListener("click", () => editor.replaceWith(body));
  const save = el("button", "", "Send");
  save.type = "button";
  save.title = "Send the edited message in a new branch; this session is kept";
  save.addEventListener("click", () => {
    const text = area.value.trim();
    if (text) resend(seq, text).catch(report);
  });
  buttons.append(cancel, save);
  editor.append(area, buttons);
  body.replaceWith(editor);
  area.focus();
}

function metaButton(label, title, handler) {
  const button = el("button", "ghost fork", label);
  button.type = "button";
  button.title = title;
  button.addEventListener("click", () => {
    Promise.resolve()
      .then(handler)
      .catch(report);
  });
  return button;
}

function entryNode(record) {
  const entry = record.entry;
  const node = el("li", "entry");
  node.dataset.seq = record.seq;
  const meta = el("div", "meta");
  const body = el("pre");
  switch (entry.kind) {
    case "turn_started":
      node.classList.add("user");
      meta.append(el("span", "", "you"));
      body.textContent = entry.input;
      break;
    case "assistant_message":
      node.classList.add("assistant");
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
  if (entry.kind === "turn_started") {
    meta.append(metaButton("edit", "Edit this message and send it in a new branch", () => editMessage(node, record.seq, entry.input)));
  }
  meta.append(metaButton("fork here", `Start a new session continuing after record ${record.seq}`, () => fork(record.seq)));
  node.append(meta, body);
  return node;
}

async function loadTranscript() {
  if (!state.current) return;
  const records = await api(`/sessions/${state.current}`);
  const list = $("transcript");
  list.replaceChildren(...records.map(entryNode).filter(Boolean));
  // Regenerate the last turn: send its message again in a new branch.
  const last = [...records].reverse().find((record) => record.entry.kind === "turn_started");
  const answer = [...list.querySelectorAll(".entry.assistant, .entry.failed")].pop();
  if (last && answer && Number(answer.dataset.seq) > last.seq) {
    const button = metaButton("regenerate", "Answer the last message again in a new branch; this session is kept", () => resend(last.seq, last.entry.input));
    answer.querySelector(".meta").insertBefore(button, answer.querySelector(".meta .fork"));
  }
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

function startTurn(input) {
  if (!input || !state.current || state.running) return false;
  if (!send({ type: "turn", session: state.current, input })) return false;
  setRunning(true);
  showLive(`you: ${input}\n`);
  return true;
}

function submit(event) {
  event.preventDefault();
  if (startTurn($("input").value.trim())) $("input").value = "";
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
      refreshStatus();
      loadSessions()
        .then(() => {
          if (event.type === "completed") awaitTitle();
        })
        .catch(report);
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
  refreshStatus();
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

// ---- Status line ----------------------------------------------------------

function statusItem(id, text, title, className) {
  const node = $(id);
  node.textContent = text;
  node.title = title || text;
  node.className = `item${className ? ` ${className}` : ""}${id === "status-root" ? " path link" : ""}`;
  node.hidden = !text;
}

function gitText(git) {
  if (!git) return ["", ""];
  if (git.error) return ["no git repository", git.error];
  const parts = [`⎇ ${git.detached ? "detached HEAD" : git.branch}`];
  const dirty = git.changed + git.untracked;
  if (dirty) parts.push(`● ${dirty}`);
  if (git.conflicts) parts.push(`⚠ ${git.conflicts} conflicts`);
  if (git.ahead) parts.push(`↑${git.ahead}`);
  if (git.behind) parts.push(`↓${git.behind}`);
  const title = [
    git.detached ? "Detached HEAD" : `Branch ${git.branch}`,
    git.upstream ? `tracking ${git.upstream} (ahead ${git.ahead}, behind ${git.behind})` : "no upstream",
    `${git.changed} changed, ${git.untracked} untracked, ${git.conflicts} conflicts`,
  ].join("\n");
  return [parts.join(" "), title];
}

function providerText(label, provider) {
  if (!provider) return [`no ${label}`, `No ${label} plugin is mounted`];
  const kind = provider.kind && provider.kind !== provider.plugin ? ` (${provider.kind})` : "";
  const model = provider.model ? ` · ${provider.model}` : "";
  return [`${label}: ${provider.plugin}${model}`, `${label} plugin ${provider.plugin}${kind}${model}`];
}

async function loadStatus() {
  const status = await api("/status");
  const workspace = status.workspace || {};
  state.workspace = workspace.root || null;
  statusItem(
    "status-root",
    workspace.root || "Choose a workspace…",
    workspace.root ? `Workspace: ${workspace.root}\nClick to change` : "Choose the folder tools work in",
  );
  const [git, gitTitle] = gitText(workspace.git);
  const dirty = workspace.git && !workspace.git.error && workspace.git.changed + workspace.git.untracked > 0;
  statusItem("status-git", git, gitTitle, dirty || workspace.git?.conflicts ? "dirty" : "");
  const [model, modelTitle] = providerText("model", status.model);
  statusItem("status-model", model, modelTitle, status.model ? "" : "bad");
  const [loop, loopTitle] = providerText("loop", status.loop);
  statusItem("status-loop", loop, loopTitle, status.loop ? "" : "bad");
  statusItem("status-tools", `${status.tools} tools`, "Tools offered to the model");
  statusItem(
    "status-policy",
    status.guarded ? "guarded" : "UNGUARDED",
    status.guarded ? "A policy plugin decides every tool call" : "No policy plugin: tool calls run without checks",
    status.guarded ? "good" : "bad",
  );
}

function refreshStatus() {
  loadStatus().catch(() => {});
}

// ---- Workspace picker -----------------------------------------------------

const RECENT_KEY = "rah-recent-workspaces";

// Browser storage is a convenience: it may be unavailable.
function recentWorkspaces() {
  try {
    const list = JSON.parse(localStorage.getItem(RECENT_KEY) || "[]");
    return Array.isArray(list) ? list.filter((item) => typeof item === "string") : [];
  } catch {
    return [];
  }
}

function rememberWorkspace(path) {
  const list = [path, ...recentWorkspaces().filter((item) => item !== path)].slice(0, 8);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    // Not remembered; nothing else depends on it.
  }
}

function folderButton(label, path, git, onClick) {
  const item = el("li");
  const button = el("button");
  button.type = "button";
  button.title = path;
  button.append(el("span", "name", label));
  if (git) button.append(el("span", "tag", "git"));
  button.addEventListener("click", onClick);
  item.append(button);
  return item;
}

function workspaceError(message) {
  $("ws-error").textContent = message || "";
  $("ws-error").hidden = !message;
}

async function browse(path) {
  workspaceError("");
  let listing;
  try {
    listing = await api(`/fs/dirs${path ? `?path=${encodeURIComponent(path)}` : ""}`);
  } catch (error) {
    workspaceError(error.message);
    return;
  }
  state.browsing = listing;
  $("ws-path").value = listing.path;
  $("ws-up").disabled = !listing.parent;
  $("ws-here").textContent = listing.git ? "Folders (this folder is a Git repository)" : "Folders";
  $("ws-roots").replaceChildren(
    ...listing.roots.map((root) => {
      const button = el("button", "ghost", root);
      button.type = "button";
      button.addEventListener("click", () => browse(root));
      return button;
    }),
  );
  const dirs = listing.dirs.map((dir) => folderButton(dir.name, dir.path, dir.git, () => browse(dir.path)));
  if (listing.truncated) dirs.push(el("li", "group", "…more folders not shown"));
  $("ws-dirs").replaceChildren(...dirs);
}

async function openWorkspacePicker() {
  const recent = recentWorkspaces();
  $("ws-recent-block").hidden = recent.length === 0;
  $("ws-recent").replaceChildren(
    ...recent.map((path) => folderButton(path, path, false, () => browse(path))),
  );
  $("workspace-dialog").showModal();
  await browse(state.workspace);
}

async function useWorkspace() {
  const path = $("ws-path").value.trim();
  if (!path) return;
  if (state.running) {
    workspaceError("Wait for the running turn to finish or cancel it first.");
    return;
  }
  $("ws-use").disabled = true;
  try {
    const result = await api("/workspace", { method: "PUT", body: JSON.stringify({ root: path }) });
    const failed = result.plugins.filter((plugin) => plugin.state !== "active");
    if (failed.length) {
      workspaceError(`Switched, but not every plugin started: ${failed.map((p) => `${p.id} (${p.state})`).join(", ")}`);
    } else {
      $("workspace-dialog").close();
    }
    rememberWorkspace(result.root);
    await loadPlugins().catch(report);
    await loadStatus().catch(report);
  } catch (error) {
    workspaceError(error.message);
  } finally {
    $("ws-use").disabled = false;
  }
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
  $("session-search").addEventListener("input", renderSessions);
  $("status-root").addEventListener("click", () => openWorkspacePicker().catch(report));
  $("ws-go").addEventListener("click", () => browse($("ws-path").value.trim()));
  $("ws-path").addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      browse($("ws-path").value.trim());
    }
  });
  $("ws-up").addEventListener("click", () => {
    if (state.browsing?.parent) browse(state.browsing.parent);
  });
  $("ws-cancel").addEventListener("click", () => $("workspace-dialog").close());
  $("ws-use").addEventListener("click", () => useWorkspace());
  $("session-rename").addEventListener("click", () => {
    const info = currentInfo();
    if (info) editTitle($("session-title"), info);
  });
  $("session-title").addEventListener("dblclick", () => $("session-rename").click());
  $("session-delete").addEventListener("click", () => {
    const info = currentInfo();
    if (info) deleteSession(info).catch(report);
  });
  for (const format of ["md", "json"]) {
    $(`export-${format}`).addEventListener("click", () => exportSession(format).catch(report));
  }
  $("toggle-plugins").addEventListener("click", () => {
    $("plugins-panel").hidden = !$("plugins-panel").hidden;
  });
  $("refresh-plugins").addEventListener("click", () => loadPlugins().catch(report));
  $("add-plugin").addEventListener("click", () => openEditor("add", null));
  $("editor").addEventListener("submit", saveEditor);
  $("editor-cancel").addEventListener("click", () => {
    $("editor").hidden = true;
  });
  setInterval(() => {
    if (!document.hidden) refreshStatus();
  }, 30000);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) refreshStatus();
  });
  try {
    await loadCatalog();
    await loadPlugins();
    refreshStatus();
    await loadSessions();
    if (state.sessions.length > 0) await openSession(state.sessions[0].id);
  } catch (error) {
    showNotice(error.message);
  }
  connect();
}

start();
