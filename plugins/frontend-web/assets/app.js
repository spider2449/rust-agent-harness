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
  // Last seen model and loop providers, to announce a switch.
  providers: {},
  // The latest status request, and the latest one shown.
  statusRequests: 0,
  statusShown: 0,
  info: null,
  // Tokens the model reported per session since this page was opened.
  usage: new Map(),
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

// Informational notices (such as a reconnected model) end with the next turn.
function showInfo(text) {
  showNotice(text);
  state.info = text;
}

function clearInfo() {
  if (state.info && $("notice").textContent === state.info) showNotice("");
  state.info = null;
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

// Images are shown separately (contentImages); the text names them.
function renderContent(output) {
  return (output.content || [])
    .map((item) => {
      if (item.type === "text") return item.value;
      if (item.type === "image") return `[${item.value.media_type} image]`;
      return JSON.stringify(item.value, null, 2);
    })
    .join("\n");
}

// Tool output is untrusted: only raster types the host accepts become a
// data URL, never SVG or HTML.
const IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"];

function contentImages(output) {
  return (output.content || [])
    .filter((item) => item.type === "image" && IMAGE_TYPES.includes(item.value.media_type) && /^[A-Za-z0-9+/]+=*$/.test(item.value.data))
    .map((item) => {
      const image = el("img", "tool-image");
      image.alt = `${item.value.media_type} image from the tool`;
      image.src = `data:${item.value.media_type};base64,${item.value.data}`;
      return image;
    });
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

// Folders as the server reports them; Windows paths ignore case.
function sameFolder(a, b) {
  return !!a && !!b && a.toLowerCase() === b.toLowerCase();
}

function folderName(path) {
  return path.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || path;
}

// Sessions are grouped by the folder they worked in: the current workspace
// first, then the other folders by their latest session, then those without
// a recorded folder. A folder shows its first sessions until "Show more".
const FOLDER_PAGE = 10;
const FOLDERS_KEY = "rah-folders";

// {key: true|false} for folders the user opened or closed; the others follow
// the default (only the current workspace is open).
function folderChoices() {
  if (!state.folderChoices) {
    try {
      state.folderChoices = JSON.parse(localStorage.getItem(FOLDERS_KEY) || "{}") || {};
    } catch {
      state.folderChoices = {};
    }
  }
  return state.folderChoices;
}

function folderKey(path) {
  return path ? path.toLowerCase() : "";
}

function folderOpen(key, here, searching) {
  if (searching) return true;
  const choice = folderChoices()[key];
  return choice === undefined ? here : choice;
}

function toggleFolder(key, open) {
  folderChoices()[key] = open;
  try {
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(folderChoices()));
  } catch {
    // Not remembered.
  }
  renderSessions();
}

function sessionGroups(visible) {
  const groups = new Map();
  const add = (path) => {
    const key = folderKey(path);
    if (!groups.has(key)) groups.set(key, { key, path, sessions: [] });
    return groups.get(key);
  };
  // The current workspace is listed even before it has sessions.
  if (state.workspace) add(state.workspace);
  for (const info of visible) add(info.workspace || null).sessions.push(info);
  const here = folderKey(state.workspace);
  const latest = (group) => Math.max(0, ...group.sessions.map((info) => info.updated || info.created));
  return [...groups.values()].sort((a, b) => {
    if ((here && a.key === here) || !b.key) return -1;
    if ((here && b.key === here) || !a.key) return 1;
    return latest(b) - latest(a);
  });
}

function folderItem(group, open, here) {
  const item = el("li", here ? "folder here" : "folder");
  item.dataset.folder = group.path || "";
  const button = el("button");
  button.type = "button";
  button.title = group.path ? `${group.path}${here ? "\nThe current workspace" : ""}` : "Sessions without a recorded folder";
  button.setAttribute("aria-expanded", String(open));
  button.append(
    el("span", "caret", open ? "▾" : "▸"),
    el("span", "folder-name", group.path ? folderName(group.path) : "No folder recorded"),
    el("span", "count", String(group.sessions.length)),
  );
  button.addEventListener("click", () => toggleFolder(group.key, !open));
  item.append(button);
  return item;
}

function renderSessions() {
  const query = $("session-search").value.trim().toLowerCase();
  const list = $("sessions");
  list.replaceChildren();
  const visible = state.sessions.filter((info) => {
    const haystack = `${info.title || ""} ${info.preview || ""} ${info.id} ${info.workspace || ""}`.toLowerCase();
    return !query || haystack.includes(query);
  });
  const expanded = state.expandedFolders || (state.expandedFolders = new Set());
  for (const group of sessionGroups(visible)) {
    const here = !!group.key && group.key === folderKey(state.workspace);
    if (query && !group.sessions.length) continue;
    const open = folderOpen(group.key, here, !!query);
    list.append(folderItem(group, open, here));
    if (!open) continue;
    if (!group.sessions.length) {
      list.append(el("li", "empty", "No sessions in this folder yet"));
      continue;
    }
    const all = query || expanded.has(group.key);
    // Keep the open session visible even past the first page.
    const shown = all ? group.sessions : group.sessions.filter((info, index) => index < FOLDER_PAGE || info.id === state.current);
    for (const info of shown) list.append(sessionItem(info));
    if (shown.length < group.sessions.length) {
      const more = el("li", "more");
      const button = el("button", "", `Show ${group.sessions.length - shown.length} more…`);
      button.type = "button";
      button.addEventListener("click", () => {
        expanded.add(group.key);
        renderSessions();
      });
      more.append(button);
      list.append(more);
    }
  }
  if (!list.children.length) list.append(el("li", "group", query ? "No matching sessions" : "No sessions yet"));
}

// Before working in a session, use the folder it belongs to, so its history
// and its tool calls refer to the same files.
async function ensureWorkspace(info) {
  if (!info?.workspace || !state.workspace || sameFolder(info.workspace, state.workspace)) return true;
  try {
    const result = await api("/workspace", { method: "PUT", body: JSON.stringify({ root: info.workspace }) });
    rememberWorkspace(result.root);
    await loadStatus().catch(() => {});
    showNotice(`Switched the workspace to ${result.root}, where this session's work happened.`);
    return true;
  } catch (error) {
    showNotice(`This session belongs to ${info.workspace}, which cannot be used now: ${error.message}`);
    return false;
  }
}

function sessionItem(info) {
  const item = el("li", info.id === state.current ? "session current" : "session");
  item.title = info.id;
  item.dataset.id = info.id;
  const name = el("span", "name", sessionName(info));
  const meta = [timeLabel(info.updated || info.created)];
  if (info.parent) meta.push(`fork of ${info.parent.session.slice(0, 8)}`);
  if (info.workspace) name.title = info.workspace;
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

function closeSession() {
  state.current = null;
  renderUsage();
  $("transcript").replaceChildren();
  $("activity").replaceChildren();
  $("input").disabled = true;
  $("send").disabled = true;
  renderHeader();
}

async function deleteSession(info) {
  if (!window.confirm(`Delete "${sessionName(info)}"?\n\nThe log is moved to the sessions' deleted folder.`)) return;
  try {
    await api(`/sessions/${info.id}`, { method: "DELETE" });
  } catch (error) {
    report(error);
    return;
  }
  if (info.id === state.current) closeSession();
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
  renderUsage();
  setRunning(false);
  await ensureWorkspace(state.sessions.find((info) => info.id === id));
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
  let images = [];
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
      images = contentImages(entry.output);
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
  node.append(meta, body, ...images);
  return node;
}

async function loadTranscript() {
  if (!state.current) return;
  const records = await api(`/sessions/${state.current}`);
  const list = $("transcript");
  list.replaceChildren(...records.map(entryNode).filter(Boolean));
  seedActivity(records);
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
  const was = state.running;
  state.running = running;
  $("send").disabled = running || !state.current;
  $("cancel").hidden = !running;
  if (running && !was) startBusy();
  if (!running) {
    stopBusy();
    state.liveText = "";
    $("live").hidden = true;
  }
}

// The busy line above the message box: what the running turn is doing and
// for how long. The tab title gets a marker too, for when it is in the
// background.
const busy = { started: 0, timer: null, tool: "" };

function startBusy() {
  busy.started = Date.now();
  setBusyPhase("Sending…");
  $("busy").hidden = false;
  tickBusy();
  clearInterval(busy.timer);
  busy.timer = setInterval(tickBusy, 1000);
  document.title = "● RAH — working";
}

function stopBusy() {
  clearInterval(busy.timer);
  busy.timer = null;
  $("busy").hidden = true;
  document.title = "RAH";
}

function tickBusy() {
  const seconds = Math.floor((Date.now() - busy.started) / 1000);
  $("busy-time").textContent = seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

function setBusyPhase(text, waiting) {
  $("busy-text").textContent = text;
  $("busy").classList.toggle("waiting", Boolean(waiting));
}

function busyEvent(event) {
  if (!state.running) return;
  // Turn events and approval requests arrive on different paths; an open
  // approval card wins until it is answered.
  if (document.querySelector("#approvals .approval")) {
    if (event.type === "tool_requested") busy.tool = event.tool_call.name;
    return;
  }
  switch (event.type) {
    case "model_request_started":
      setBusyPhase("Waiting for the model…");
      break;
    case "model_reasoning_started":
      setBusyPhase("Thinking…");
      break;
    case "model_delta":
      setBusyPhase("Writing…");
      break;
    case "tool_requested":
      busy.tool = event.tool_call.name;
      setBusyPhase(`Running ${busy.tool}…`);
      break;
    case "tool_started":
      // Only an id here; the name came with `tool_requested`.
      setBusyPhase(`Running ${busy.tool || "a tool"}…`);
      break;
    case "approval_required":
      setBusyPhase("Waiting for your approval", true);
      break;
    case "tool_finished":
      setBusyPhase("Waiting for the model…");
      break;
    default:
      break;
  }
}

function showLive(text) {
  state.liveText += text;
  const live = $("live");
  // Follow the output unless the user scrolled up to read.
  const following = live.hidden || live.scrollHeight - live.scrollTop - live.clientHeight < 24;
  live.textContent = state.liveText;
  live.hidden = false;
  if (following) live.scrollTop = live.scrollHeight;
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

async function startTurn(input) {
  if (!input || !state.current || state.running || state.starting) return false;
  state.starting = true;
  try {
    // Another tab may have moved the workspace since the session was opened.
    if (!(await ensureWorkspace(currentInfo()))) return false;
    if (!send({ type: "turn", session: state.current, input })) return false;
    clearInfo();
    setRunning(true);
    showLive(`you: ${input}\n`);
    return true;
  } finally {
    state.starting = false;
  }
}

// Slash commands in the message box. `/call` and `/notool` are turns for the
// mock model; the others run in the browser. Typing `/` lists them.
const SLASH_COMMANDS = [
  { name: "/new", usage: "/new", help: "Start a new session" },
  { name: "/plugins", usage: "/plugins", help: "Show the Plugins panel" },
  {
    name: "/stop", usage: "/stop <plugin>", help: "Stop a plugin, keeping its configuration",
    // Policy plugins guard tool calls; the server refuses to stop them.
    plugin: (p) => p.state !== "stopped" && p.kind && !p.locked && p.kind !== "policy-authority",
  },
  { name: "/start", usage: "/start <plugin>", help: "Start a stopped plugin", plugin: (p) => p.state === "stopped" },
  { name: "/restart", usage: "/restart <plugin>", help: "Restart a plugin", plugin: (p) => p.state !== "stopped" && p.kind && !p.locked },
  { name: "/cancel", usage: "/cancel", help: "Cancel the running turn" },
  { name: "/help", usage: "/help", help: "List the commands" },
  { name: "/call", usage: "/call <tool> <json>", help: "Run a turn that requests a tool (mock model)", turn: true },
  { name: "/notool", usage: "/notool <text>", help: "Run a turn without tools (mock model)", turn: true },
];

const slash = { items: [], index: 0, plugins: null, loading: false };

function slashCommand(name) {
  return SLASH_COMMANDS.find((command) => command.name === name);
}

// Plugin ids for completing `/stop` and friends, loaded on first use.
function slashPlugins() {
  if (slash.plugins || slash.loading) return slash.plugins || [];
  slash.loading = true;
  api("/plugins")
    .then((plugins) => {
      slash.plugins = plugins;
      updateSlashMenu();
    })
    .catch(() => {})
    .finally(() => {
      slash.loading = false;
    });
  return [];
}

function slashItems(value) {
  if (!value.startsWith("/") || value.includes("\n")) return [];
  const space = value.indexOf(" ");
  if (space < 0) {
    return SLASH_COMMANDS.filter((command) => command.name.startsWith(value)).map((command) => ({
      label: command.usage,
      detail: command.help,
      insert: command.plugin || command.turn ? `${command.name} ` : command.name,
    }));
  }
  const command = slashCommand(value.slice(0, space));
  const argument = value.slice(space + 1);
  if (!command || !command.plugin || argument.includes(" ")) return [];
  return slashPlugins()
    .filter((plugin) => command.plugin(plugin) && plugin.id.startsWith(argument))
    .map((plugin) => ({ label: plugin.id, detail: plugin.state, insert: `${command.name} ${plugin.id}` }));
}

function hideSlashMenu() {
  slash.items = [];
  $("slash-menu").hidden = true;
  $("input").removeAttribute("aria-activedescendant");
}

function updateSlashMenu() {
  const value = $("input").value;
  if (!value.startsWith("/")) slash.plugins = null; // Reload on the next command.
  slash.items = slashItems(value);
  slash.index = Math.min(slash.index, Math.max(slash.items.length - 1, 0));
  const menu = $("slash-menu");
  if (!slash.items.length) {
    hideSlashMenu();
    return;
  }
  const rows = slash.items.map((item, index) => {
    const row = el("li", `slash-item${index === slash.index ? " active" : ""}`);
    row.id = `slash-${index}`;
    row.setAttribute("role", "option");
    row.setAttribute("aria-selected", String(index === slash.index));
    row.append(el("code", "", item.label), el("span", "", item.detail));
    // Keep the focus in the message box.
    row.addEventListener("mousedown", (event) => {
      event.preventDefault();
      acceptSlash(index);
    });
    return row;
  });
  menu.replaceChildren(el("li", "slash-hint", "Tab or Enter to complete, Esc to close"), ...rows);
  menu.hidden = false;
  $("input").setAttribute("aria-activedescendant", `slash-${slash.index}`);
  $(`slash-${slash.index}`)?.scrollIntoView({ block: "nearest" });
}

function acceptSlash(index) {
  const item = slash.items[index];
  if (!item) return;
  $("input").value = item.insert;
  $("input").focus();
  slash.index = 0;
  updateSlashMenu();
}

// Handles menu keys. Returns whether the key was used.
function slashKey(event) {
  if ($("slash-menu").hidden || !slash.items.length) return false;
  const last = slash.items.length - 1;
  switch (event.key) {
    case "ArrowDown":
      slash.index = slash.index >= last ? 0 : slash.index + 1;
      break;
    case "ArrowUp":
      slash.index = slash.index <= 0 ? last : slash.index - 1;
      break;
    case "Escape":
      hideSlashMenu();
      break;
    case "Tab":
      acceptSlash(slash.index);
      break;
    case "Enter": {
      if (event.shiftKey) return false;
      // A complete command is sent; a partial one is completed first.
      const item = slash.items[slash.index];
      if (item.insert === $("input").value) return false;
      acceptSlash(slash.index);
      break;
    }
    default:
      return false;
  }
  event.preventDefault();
  if (event.key.startsWith("Arrow")) updateSlashMenu();
  return true;
}

// Runs a local slash command. Returns whether `input` was one (handled or
// rejected); turn commands and plain text return false.
async function runSlash(input) {
  if (!input.startsWith("/")) return false;
  const [name, ...rest] = input.split(/\s+/);
  const argument = rest.join(" ");
  const command = slashCommand(name);
  if (command?.turn) return false;
  hideSlashMenu();
  if (!command) {
    showNotice(`Unknown command ${name}. Type / to see the commands.`);
    return true;
  }
  if (command.plugin && (!argument || argument.includes(" "))) {
    showNotice(`Usage: ${command.usage}`);
    return true;
  }
  const path = `/plugins/${encodeURIComponent(argument)}`;
  try {
    switch (name) {
      case "/new":
        await newSession();
        break;
      case "/plugins":
        showSide("plugins");
        break;
      case "/stop": {
        const plugin = (slash.plugins || []).find((p) => p.id === argument);
        if (plugin?.kind === "frontend-web") {
          stopPlugin(plugin);
          break;
        }
        await manage(`${path}/stop`, "POST");
        showInfo(`${argument}: stopped`);
        break;
      }
      case "/start":
      case "/restart": {
        const result = await manage(`${path}/${name.slice(1)}`, "POST");
        if (result.state === "active") showInfo(`${argument}: active`);
        break;
      }
      case "/cancel":
        if (state.running) send({ type: "cancel" });
        else showNotice("No turn is running.");
        break;
      case "/help":
        $("input").value = "/";
        updateSlashMenu();
        return "keep";
    }
  } catch (error) {
    report(error);
  }
  slash.plugins = null;
  return true;
}

async function submit(event) {
  event.preventDefault();
  const input = $("input").value.trim();
  const local = await runSlash(input);
  if (local) {
    if (local !== "keep") $("input").value = "";
    return;
  }
  if (await startTurn(input)) $("input").value = "";
}

function addUsage(session, usage) {
  const total = state.usage.get(session) || { input: 0, output: 0, cached: 0, requests: 0 };
  total.input += usage.input_tokens || 0;
  total.output += usage.output_tokens || 0;
  total.cached += usage.cached_input_tokens || 0;
  total.requests += 1;
  state.usage.set(session, total);
}

function formatTokens(count) {
  return count >= 10000 ? `${(count / 1000).toFixed(count >= 100000 ? 0 : 1)}k` : String(count);
}

function renderUsage() {
  const item = $("status-usage");
  const total = state.current && state.usage.get(state.current);
  item.hidden = !total;
  if (!total) return;
  item.textContent = `${formatTokens(total.input)} in · ${formatTokens(total.output)} out`;
  item.title = `Tokens the model reported for this session since the page was opened: `
    + `${total.input} input (${total.cached} cached), ${total.output} output, `
    + `${total.requests} model request${total.requests === 1 ? "" : "s"}`;
}

function onEvent(session, event) {
  if (event.type === "model_usage") {
    addUsage(session, event.usage);
    if (session === state.current) renderUsage();
    return;
  }
  if (session !== state.current) return;
  eventActivity(event);
  busyEvent(event);
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
      reloadFiles().catch(report);
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

// ---- Workspace panel --------------------------------------------------------

// The right-hand panel: the workspace's files (read-only), the files opened
// from them, and an activity log of the open session; or the plugins.
const SIDE_KEY = "rah-side";
const files = {
  root: null,
  // Directory path ("" is the root) -> its listing, once loaded.
  dirs: new Map(),
  expanded: new Set([""]),
  // Open files in tab order: {path, data, error}.
  open: [],
  current: null,
};

function sidePrefs() {
  try {
    return JSON.parse(localStorage.getItem(SIDE_KEY) || "{}") || {};
  } catch {
    return {};
  }
}

function saveSidePrefs(change) {
  try {
    localStorage.setItem(SIDE_KEY, JSON.stringify({ ...sidePrefs(), ...change }));
  } catch {
    // Not remembered.
  }
}

function sideView() {
  return $("plugins-panel").hidden ? "files" : "plugins";
}

function showSide(view) {
  $("side").hidden = false;
  $("files-view").hidden = view !== "files";
  $("plugins-panel").hidden = view !== "plugins";
  for (const tab of $("side-tabs").querySelectorAll("[data-view]")) {
    const current = tab.dataset.view === view;
    tab.classList.toggle("current", current);
    tab.setAttribute("aria-selected", String(current));
  }
  saveSidePrefs({ open: true, view });
  if (view === "files" && !files.dirs.has("")) loadDir("").catch(treeError);
  if (view === "files" && !git.loaded) loadGit();
  if (view === "plugins") loadPlugins().catch(report);
}

function hideSide() {
  $("side").hidden = true;
  saveSidePrefs({ open: false });
}

// The header buttons open their view, or close the panel when it shows it.
function toggleSide(view) {
  if (!$("side").hidden && sideView() === view) hideSide();
  else showSide(view);
}

function setupResizer() {
  const width = sidePrefs().width;
  if (width) document.documentElement.style.setProperty("--side-width", `${width}px`);
  const handle = $("side-resizer");
  handle.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    handle.classList.add("dragging");
    const move = (moved) => {
      const next = Math.round(Math.min(Math.max(window.innerWidth - moved.clientX, 320), window.innerWidth * 0.75));
      document.documentElement.style.setProperty("--side-width", `${next}px`);
    };
    const done = () => {
      handle.classList.remove("dragging");
      handle.removeEventListener("pointermove", move);
      saveSidePrefs({ width: Math.round($("side").getBoundingClientRect().width) });
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", done, { once: true });
    handle.addEventListener("pointercancel", done, { once: true });
  });
}

function treeError(error) {
  $("tree").replaceChildren(el("li", "note", error.message));
}

async function loadDir(path) {
  const listing = await api(`/files?path=${encodeURIComponent(path)}`);
  if (path === "") {
    files.root = listing.root;
    $("tree-root").textContent = folderName(listing.root);
    $("tree-root").title = listing.root;
  }
  files.dirs.set(path, listing);
  renderTree();
}

// A turn may have changed files: reload the open folders and files.
async function reloadFiles() {
  if (!files.dirs.has("")) {
    if (git.loaded) await loadGit();
    return;
  }
  const paths = [...files.expanded].filter((path) => files.dirs.has(path));
  files.dirs.clear();
  for (const path of paths) {
    try {
      await loadDir(path);
    } catch {
      // Gone: shown closed.
      files.expanded.delete(path);
    }
  }
  renderTree();
  for (const file of files.open) await readFile(file);
  renderFile();
  await loadGit();
}

// The workspace moved: start over in the new folder.
function resetFiles() {
  files.root = null;
  files.dirs.clear();
  files.expanded = new Set([""]);
  files.open = [];
  files.current = null;
  $("tree-root").textContent = "Files";
  $("tree-root").title = "";
  $("tree").replaceChildren();
  renderFile();
  git.loaded = false;
  renderGit();
  if (!$("side").hidden && sideView() === "files") {
    loadDir("").catch(treeError);
    loadGit();
  }
}

function treeNote(text, depth) {
  const note = el("li", "note", text);
  note.style.setProperty("--depth", depth);
  return note;
}

function treeNodes(path, depth) {
  const listing = files.dirs.get(path);
  if (!listing) return [treeNote("Loading…", depth)];
  const nodes = listing.entries.map((entry) => {
    const item = el("li", entry.dir ? "dir" : "file");
    item.setAttribute("role", "treeitem");
    const button = el("button");
    button.type = "button";
    button.title = entry.path;
    button.dataset.path = entry.path;
    button.style.setProperty("--depth", depth);
    const open = entry.dir && files.expanded.has(entry.path);
    if (entry.dir) item.setAttribute("aria-expanded", String(open));
    if (!entry.dir && entry.path === files.current) button.classList.add("open-file");
    button.append(
      el("span", "caret", entry.dir ? (open ? "▾" : "▸") : ""),
      el("span", "icon-glyph", entry.dir ? "▣" : "▢"),
      el("span", "", entry.name),
    );
    button.addEventListener("click", () => {
      if (!entry.dir) {
        openFile(entry.path).catch(report);
        return;
      }
      if (open) files.expanded.delete(entry.path);
      else files.expanded.add(entry.path);
      if (!open && !files.dirs.has(entry.path)) loadDir(entry.path).catch(report);
      renderTree();
    });
    item.append(button);
    if (open) {
      const children = el("ul");
      children.setAttribute("role", "group");
      children.append(...treeNodes(entry.path, depth + 1));
      item.append(children);
    }
    return item;
  });
  if (!listing.entries.length) nodes.push(treeNote("Empty folder", depth));
  if (listing.truncated) nodes.push(treeNote("…more entries not shown", depth));
  return nodes;
}

function renderTree() {
  const tree = $("tree");
  const scroll = tree.scrollTop;
  tree.replaceChildren(...treeNodes("", 0));
  tree.scrollTop = scroll;
}

// An open tab is a file or a file's diff: {key, kind, path, label, data,
// error}. A file's key is its path, so the tree can mark it.
const TAB_SOURCES = {
  file: (tab) => `/files/content?path=${encodeURIComponent(tab.path)}`,
  diff: (tab) => `/git/diff?path=${encodeURIComponent(tab.path)}`,
};

async function readFile(file) {
  try {
    file.data = await api(TAB_SOURCES[file.kind](file));
    file.error = null;
  } catch (error) {
    file.data = null;
    file.error = error.message;
  }
}

async function openTab(kind, path, label) {
  const key = kind === "file" ? path : `${kind}:${path}`;
  let file = files.open.find((open) => open.key === key);
  if (!file) {
    file = { key, kind, path, label, data: null, error: null };
    files.open.push(file);
    await readFile(file);
  }
  files.current = key;
  if ($("side").hidden || sideView() !== "files") showSide("files");
  renderFile();
  renderTree();
  renderGit();
}

function openFile(path) {
  return openTab("file", path, folderName(path));
}

function openDiff(path) {
  return openTab("diff", path, `${folderName(path)} (diff)`);
}

function closeFile(key) {
  const index = files.open.findIndex((file) => file.key === key);
  if (index < 0) return;
  files.open.splice(index, 1);
  if (files.current === key) files.current = files.open[Math.min(index, files.open.length - 1)]?.key || null;
  renderFile();
  renderTree();
  renderGit();
}

function closeAllFiles() {
  files.open = [];
  files.current = null;
  renderFile();
  renderTree();
  renderGit();
}

// File contents are untrusted text: shown as text nodes and token spans
// (Highlight), never as HTML.
function codeView(text, path) {
  const lines = (text.endsWith("\n") ? text.slice(0, -1) : text).split("\n");
  const code = el("div", "code");
  const body = el("pre", "lines");
  body.append(Highlight.render(lines.join("\n"), path));
  code.append(el("pre", "gutter", lines.map((_, index) => index + 1).join("\n")), body);
  return code;
}

function fileTab(file) {
  const current = file.key === files.current;
  const tab = el("div", current ? `file-tab current ${file.kind}` : `file-tab ${file.kind}`);
  tab.setAttribute("role", "tab");
  tab.setAttribute("aria-selected", String(current));
  tab.title = file.kind === "file" ? file.path : `${file.label}: ${file.path}`;
  tab.dataset.key = file.key;
  const close = el("button", "", "✕");
  close.type = "button";
  close.title = "Close";
  close.setAttribute("aria-label", `Close ${file.label}`);
  close.addEventListener("click", (event) => {
    event.stopPropagation();
    closeFile(file.key);
  });
  tab.append(el("span", "", file.label), close);
  tab.addEventListener("click", () => {
    files.current = file.key;
    renderFile();
    renderTree();
    renderGit();
  });
  return tab;
}

function renderFile() {
  $("file-tabs").replaceChildren(...files.open.map(fileTab));
  $("close-all").hidden = !files.open.length;
  const view = $("file-view");
  const file = files.open.find((open) => open.key === files.current);
  if (!file) {
    view.replaceChildren(el("div", "placeholder", "Choose a file to view it here (read-only)."));
  } else if (file.error) {
    view.replaceChildren(el("div", "placeholder", file.error));
  } else if (file.kind === "file" && file.data.binary) {
    view.replaceChildren(el("div", "placeholder", `Binary file, ${file.data.size} bytes: not shown.`));
  } else if (file.kind !== "file" && !file.data.text) {
    view.replaceChildren(el("div", "placeholder", "No changes."));
  } else {
    const parts = [];
    if (file.data.truncated) {
      const size = file.kind === "file" ? ` ${file.data.size}-byte` : "";
      parts.push(el("div", "file-note", `Showing the start of this${size} ${file.kind === "file" ? "file" : "diff"}.`));
    }
    parts.push(codeView(file.data.text, file.kind === "file" ? file.path : "view.diff"));
    view.replaceChildren(...parts);
  }
}

// ---- Source control ---------------------------------------------------------

// The Git state under the file tree, read-only: the branch and the changed
// files, grouped by folder. Paths are untrusted text.
const git = { loaded: false, loading: null, branch: null, changes: [], truncated: false, error: null, signature: null };

const STATUS_NAMES = {
  M: "modified", A: "added", D: "deleted", R: "renamed", C: "copied", T: "type changed", U: "untracked", "!": "conflicted",
};

async function loadGit() {
  if (git.loading) return git.loading;
  git.loading = (async () => {
    try {
      const changes = await api("/git/changes");
      // Not a repository (or no Git): a state, not a failure.
      if (changes.error) throw new Error(changes.error);
      Object.assign(git, { branch: changes.branch, changes: changes.changes, truncated: changes.truncated, error: null });
    } catch (error) {
      Object.assign(git, { branch: null, changes: [], truncated: false, error: error.message });
    } finally {
      git.loaded = true;
      git.loading = null;
    }
    renderGit();
  })();
  return git.loading;
}

function changeItem(change, nested) {
  const item = el("li", "change");
  const button = el("button");
  button.type = "button";
  button.dataset.path = change.path;
  const opens = change.status === "U" ? change.path : `diff:${change.path}`;
  if (files.current === opens) button.classList.add("open-file");
  const name = STATUS_NAMES[change.status] || change.status;
  button.title = `${change.path}\n${name}${change.staged ? " (staged)" : ""}${change.orig ? `\nfrom ${change.orig}` : ""}`;
  button.style.setProperty("--depth", nested ? 1 : 0);
  button.append(
    el("span", "change-name", folderName(change.path)),
    el("span", `badge-status status-${change.status === "!" ? "conflict" : change.status}`, change.status),
  );
  // Untracked files have no diff: their content is the change.
  button.addEventListener("click", () => (change.status === "U" ? openFile(change.path) : openDiff(change.path)).catch(report));
  item.append(button);
  return item;
}

function renderGit() {
  const branch = git.branch && git.branch !== "(detached)" ? `⎇ ${git.branch}` : git.branch ? "⎇ detached" : "";
  $("git-branch").textContent = branch;
  $("changes-count").textContent = git.loaded && !git.error ? String(git.changes.length) : "";
  const changes = $("changes");
  if (!git.loaded) {
    changes.replaceChildren(el("li", "note", "Loading…"));
    return;
  }
  if (git.error) {
    changes.replaceChildren(el("li", "note", git.error));
    return;
  }
  // One row per folder, then its files; files at the root come last.
  const folders = new Map();
  for (const change of git.changes) {
    const cut = change.path.lastIndexOf("/");
    const folder = cut < 0 ? "" : change.path.slice(0, cut);
    if (!folders.has(folder)) folders.set(folder, []);
    folders.get(folder).push(change);
  }
  const names = [...folders.keys()].sort((a, b) => (!a ? 1 : !b ? -1 : a.localeCompare(b)));
  const rows = [];
  for (const folder of names) {
    if (folder) {
      const row = el("li", "change-folder", folder);
      row.title = folder;
      rows.push(row);
    }
    for (const change of folders.get(folder)) rows.push(changeItem(change, !!folder));
  }
  if (!rows.length) rows.push(el("li", "note", "No changes"));
  if (git.truncated) rows.push(el("li", "note", "…more changes not shown"));
  changes.replaceChildren(...rows);
}

// The status line's Git summary changed (every 30 seconds it is read): the
// list is probably stale.
function gitSignature(state) {
  const signature = state ? JSON.stringify([state.branch, state.changed, state.untracked, state.conflicts, state.ahead, state.error]) : "";
  if (signature !== git.signature) {
    const known = git.signature !== null;
    git.signature = signature;
    if (known && git.loaded && !$("side").hidden) loadGit();
  }
}

function setupSplitter() {
  const pane = $("tree-pane");
  const apply = (ratio) => pane.style.setProperty("--tree-ratio", String(ratio));
  const saved = Number(sidePrefs().treeRatio);
  if (saved > 0 && saved < 1) apply(saved);
  const handle = $("pane-splitter");
  handle.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    handle.classList.add("dragging");
    let ratio = null;
    const move = (moved) => {
      const box = pane.getBoundingClientRect();
      ratio = Math.min(Math.max((moved.clientY - box.top) / box.height, 0.1), 0.9);
      apply(ratio.toFixed(3));
    };
    const done = () => {
      handle.classList.remove("dragging");
      handle.removeEventListener("pointermove", move);
      if (ratio !== null) saveSidePrefs({ treeRatio: Number(ratio.toFixed(3)) });
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", done, { once: true });
    handle.addEventListener("pointercancel", done, { once: true });
  });
  // Keyboard: arrows move the split by 5%.
  handle.addEventListener("keydown", (event) => {
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    event.preventDefault();
    const current = Number(getComputedStyle(pane).getPropertyValue("--tree-ratio")) || 0.6;
    const next = Math.min(Math.max(current + (event.key === "ArrowUp" ? -0.05 : 0.05), 0.1), 0.9);
    apply(next.toFixed(3));
    saveSidePrefs({ treeRatio: Number(next.toFixed(3)) });
  });
}

// ---- Activity ---------------------------------------------------------------

const ACTIVITY_LIMIT = 500;

function clock() {
  return new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

// Tool arguments are untrusted: shown as text, and a `path` among them only
// opens the read-only view, which the server keeps inside the workspace.
function logActivity(text, kind, when, path) {
  const log = $("activity");
  const following = log.scrollHeight - log.scrollTop - log.clientHeight < 24;
  const item = el("li", kind || "");
  const line = el("span", "text", text);
  line.title = text;
  item.append(el("span", "time", when || clock()), line);
  if (path) {
    item.classList.add("link");
    item.title = `Open ${path}`;
    item.addEventListener("click", () => openFile(path).catch(report));
  }
  log.append(item);
  while (log.children.length > ACTIVITY_LIMIT) log.firstChild.remove();
  if (following) log.scrollTop = log.scrollHeight;
}

function oneLine(text, limit = 160) {
  const line = String(text).replace(/\s+/g, " ").trim();
  return line.length > limit ? `${line.slice(0, limit)}…` : line;
}

function toolPath(input) {
  return input && typeof input.path === "string" && input.path ? input.path : null;
}

function toolActivity(name, input, when) {
  logActivity(`⚙ ${name} ${oneLine(JSON.stringify(input))}`, "tool", when, toolPath(input));
}

function resultActivity(output, effect, when) {
  const status = output.is_error ? "✗ error" : "✓ done";
  const uncertain = effect === "uncertain";
  const kind = output.is_error ? "bad" : uncertain ? "warn" : "muted";
  logActivity(`  ${status}${uncertain ? " (effect uncertain)" : ""} ${oneLine(renderContent(output), 120)}`, kind, when);
}

// The open session's history in brief, by record number.
function seedActivity(records) {
  $("activity").replaceChildren();
  for (const record of records) {
    const entry = record.entry;
    const when = `#${record.seq}`;
    if (entry.kind === "turn_started") logActivity(`▶ ${oneLine(entry.input)}`, "", when);
    else if (entry.kind === "tool_call") toolActivity(entry.name, entry.arguments, when);
    else if (entry.kind === "tool_result") resultActivity(entry.output, entry.effect, when);
    else if (entry.kind === "turn_completed") logActivity("■ turn completed", "good", when);
    else if (entry.kind === "turn_failed") logActivity(`■ turn failed: ${oneLine(entry.message)}`, "bad", when);
  }
}

function eventActivity(event) {
  switch (event.type) {
    case "model_request_started":
      logActivity("… model request", "muted");
      break;
    case "tool_requested":
      toolActivity(event.tool_call.name, event.tool_call.input);
      break;
    case "approval_required":
      logActivity("? waiting for approval", "warn");
      break;
    case "tool_finished":
      resultActivity(event.output);
      break;
    case "completed":
      logActivity("■ turn completed", "good");
      break;
    case "failed":
      logActivity(`■ turn failed: ${oneLine(event.message)}`, "bad");
      break;
    case "cancelled":
      logActivity("■ turn cancelled", "warn");
      break;
    default:
      break;
  }
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
    else if (data.type === "approval_required") {
      showApproval(data);
      if (state.running) setBusyPhase("Waiting for your approval", true);
    } else if (data.type === "approval_resolved") {
      $(`approval-${data.id}`)?.remove();
      if (state.running) setBusyPhase(`Running ${busy.tool || "a tool"}…`);
    }
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
  if (result.state && !["active", "unmounted", "stopped"].includes(result.state)) {
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

function stopPlugin(plugin) {
  if (plugin.kind === "frontend-web" && !window.confirm(`Stop ${plugin.id}?

This is the Web UI you are using; it will stop responding.`)) return;
  manage(`/plugins/${encodeURIComponent(plugin.id)}/stop`, "POST").catch(report);
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
    const path = `/plugins/${encodeURIComponent(plugin.id)}`;
    if (plugin.state === "stopped") {
      action("Start", () => manage(`${path}/start`, "POST").catch(report));
    } else {
      // Policy plugins guard tool calls; the server refuses to stop them.
      if (plugin.kind !== "policy-authority") action("Stop", () => stopPlugin(plugin));
      action("Restart", () => manage(`${path}/restart`, "POST").catch(report));
      action("Configure", () => openEditor("configure", plugin));
      action("Replace", () => openEditor("replace", plugin));
    }
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

// A replaced provider reconnects without any visible sign, so say so once
// the status line sees a different one. The first status only records it.
function announceProvider(label, provider) {
  const key = provider ? [provider.plugin, provider.kind, provider.model].join("\n") : "";
  const previous = state.providers[label];
  state.providers[label] = key;
  if (previous === undefined || previous === key) return;
  if (!provider) {
    showNotice(`No ${label} plugin is active: messages cannot run until one is.`);
    return;
  }
  const model = provider.model ? ` · ${provider.model}` : "";
  const verb = label === "model" ? "Reconnected to" : "Switched to";
  showInfo(`${verb} ${label} ${provider.plugin}${model}. The next message uses it.`);
}

async function loadStatus() {
  // Overlapping requests can answer out of order; an older answer would
  // undo a newer one and announce a switch back.
  const request = ++state.statusRequests;
  const status = await api("/status");
  if (request < state.statusShown) return;
  state.statusShown = request;
  const workspace = status.workspace || {};
  // Undefined until the first status: nothing has moved yet.
  const moved = state.workspace !== undefined && !sameFolder(state.workspace, workspace.root) && (state.workspace || workspace.root);
  state.workspace = workspace.root || null;
  if (moved) {
    resetFiles();
    renderSessions();
  }
  statusItem(
    "status-root",
    workspace.root || "Choose a workspace…",
    workspace.root ? `Workspace: ${workspace.root}\nClick to change` : "Choose the folder tools work in",
  );
  gitSignature(workspace.git);
  const [gitLabel, gitTitle] = gitText(workspace.git);
  const dirty = workspace.git && !workspace.git.error && workspace.git.changed + workspace.git.untracked > 0;
  statusItem("status-git", gitLabel, gitTitle, dirty || workspace.git?.conflicts ? "dirty" : "");
  const [model, modelTitle] = providerText("model", status.model);
  statusItem("status-model", model, modelTitle, status.model ? "" : "bad");
  const [loop, loopTitle] = providerText("loop", status.loop);
  statusItem("status-loop", loop, loopTitle, status.loop ? "" : "bad");
  announceProvider("model", status.model);
  announceProvider("loop", status.loop);
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
    // The open session belongs to the folder just left; close it.
    const info = currentInfo();
    if (info?.workspace && !sameFolder(info.workspace, result.root)) closeSession();
    await loadSessions().catch(report);
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
    if (slashKey(event)) return;
    if (event.key === "Enter" && !event.shiftKey) submit(event);
  });
  $("input").addEventListener("input", updateSlashMenu);
  $("input").addEventListener("blur", hideSlashMenu);
  $("cancel").addEventListener("click", () => send({ type: "cancel" }));
  $("new-session").addEventListener("click", () => newSession().catch((error) => showNotice(error.message)));
  setupSettings();
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
  $("toggle-plugins").addEventListener("click", () => toggleSide("plugins"));
  $("toggle-side").addEventListener("click", () => toggleSide("files"));
  for (const tab of $("side-tabs").querySelectorAll("[data-view]")) {
    tab.addEventListener("click", () => showSide(tab.dataset.view));
  }
  $("side-close").addEventListener("click", hideSide);
  $("tree-refresh").addEventListener("click", () => reloadFiles().catch(report));
  $("close-all").addEventListener("click", closeAllFiles);
  $("activity-clear").addEventListener("click", () => $("activity").replaceChildren());
  setupResizer();
  setupSplitter();
  $("git-refresh").addEventListener("click", () => loadGit());
  renderFile();
  renderGit();
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
  const side = sidePrefs();
  if (side.open ?? window.innerWidth >= 1100) showSide(side.view === "plugins" ? "plugins" : "files");
  try {
    await loadCatalog();
    await loadPlugins();
    // The workspace decides which sessions are listed and opened first.
    await loadStatus().catch(() => {});
    await loadSessions();
    const first = state.sessions.find((info) => !state.workspace || sameFolder(info.workspace, state.workspace));
    if (first) await openSession(first.id);
  } catch (error) {
    showNotice(error.message);
  }
  connect();
}

start();
