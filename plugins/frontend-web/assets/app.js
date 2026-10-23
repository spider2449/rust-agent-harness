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
  // Preferences kept by the server (`/api/preferences`): display, the
  // folders opened in the session list, recent workspaces, the side panel.
  prefs: { raw_markdown: false },
  // `GET /api/modes`: `{modes, active}`; one mode is active for every window.
  modes: { modes: [], active: null },
  // `GET /api/groups`: the API groups (tools-http instances): `{id, label, active}`.
  groups: [],
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
  $("notice-text").textContent = text;
  $("notice").hidden = !text;
}

// A warning that clears itself: nothing in it needs an answer, and it is
// never written to a session.
let noticeTimer = null;
function showTransientNotice(text, milliseconds = 8000) {
  showNotice(text);
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => {
    if ($("notice-text").textContent === text) showNotice("");
  }, milliseconds);
}

// Informational notices (such as a reconnected model) end with the next turn.
function showInfo(text) {
  showNotice(text);
  state.info = text;
}

function clearInfo() {
  if (state.info && $("notice-text").textContent === state.info) showNotice("");
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

// Saves `state.prefs` on the server, once changes stop for a moment (a drag
// saves once). Not remembered when the save fails; nothing depends on it.
let prefsTimer = null;
function savePrefs() {
  clearTimeout(prefsTimer);
  prefsTimer = setTimeout(() => {
    api("/preferences", { method: "PUT", body: JSON.stringify(state.prefs) }).catch(() => {});
  }, 300);
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

// {key: true|false} for folders the user opened or closed; the others follow
// the default (only the current workspace is open).
function folderChoices() {
  if (!state.prefs.folders || typeof state.prefs.folders !== "object") state.prefs.folders = {};
  return state.prefs.folders;
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
  savePrefs();
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
  const button = el("button", "folder-toggle");
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
  if (group.sessions.length) {
    const actions = el("span", "item-actions");
    const remove = el("button", "ghost icon", "✕");
    remove.type = "button";
    remove.title = "Delete all sessions in this folder";
    remove.setAttribute("aria-label", "Delete folder sessions");
    remove.addEventListener("click", (event) => {
      event.stopPropagation();
      deleteFolder(group).catch(report);
    });
    actions.append(remove);
    item.append(actions);
  }
  return item;
}

// Deletes every session of a folder in the list, also those a search hides.
// The folder itself and its files are not touched.
async function deleteFolder(group) {
  const infos = state.sessions.filter((info) => folderKey(info.workspace || null) === group.key);
  if (!infos.length) return;
  const name = group.path || "No folder recorded";
  const count = infos.length === 1 ? "the 1 session" : `all ${infos.length} sessions`;
  const question = `Delete ${count} in "${name}"?

The logs are moved to the sessions' deleted folder. The folder and its files are not touched.`;
  if (!window.confirm(question)) return;
  const result = await api("/sessions/delete", {
    method: "POST",
    body: JSON.stringify({ ids: infos.map((info) => info.id) }),
  });
  if (result.deleted.includes(state.current)) closeSession();
  await loadSessions();
  if (result.failed.length) {
    const first = result.failed[0].error;
    showNotice(`${result.failed.length} of ${infos.length} sessions were not deleted: ${first}`);
  }
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
    state.workspaceInfo = result.workspace;
    rememberWorkspace(result.workspace);
    await loadStatus().catch(() => {});
    const kept = result.workspace?.folders?.length > 1 ? " (a folder of this workspace)" : "";
    showTransientNotice(`Switched the active folder to ${result.root}${kept}, where this session's work happened.`);
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

const EXPORT_TYPES = {
  md: { description: "Markdown", accept: { "text/markdown": [".md"] } },
  json: { description: "JSON", accept: { "application/json": [".json"] } },
};

// Exports go through fetch, since the API needs the token header. Where the
// browser has a save dialog (File System Access API), the user picks the
// file; otherwise the browser downloads it to its download folder.
async function exportSession(format) {
  const name = `rah-session-${state.current}.${format}`;
  let handle = null;
  if (window.showSaveFilePicker) {
    // Asked first, while the click still counts as a user gesture.
    try {
      handle = await window.showSaveFilePicker({
        suggestedName: name,
        types: [EXPORT_TYPES[format]],
      });
    } catch (error) {
      if (error.name === "AbortError") return;
      throw error;
    }
  }
  const response = await fetch(`/api/sessions/${state.current}/export?format=${format}`, {
    headers: { Authorization: `Bearer ${state.token}` },
  });
  if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  const blob = await response.blob();
  if (handle) {
    const writable = await handle.createWritable();
    try {
      await writable.write(blob);
    } catch (error) {
      await writable.abort();
      throw error;
    }
    await writable.close();
    return;
  }
  const url = URL.createObjectURL(blob);
  const link = el("a");
  link.href = url;
  link.download = name;
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
  let body = el("pre");
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
      if (state.prefs.raw_markdown) {
        body.textContent = entry.text;
      } else {
        // Rendered Markdown (see markdown.js); the source is untrusted.
        body = el("div", "md");
        body.append(Markdown.render(entry.text));
      }
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
    case "model_retrying":
      setBusyPhase(`Model busy, retry ${event.attempt} of ${event.retries}…`);
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
    case "model_retrying":
      showLive(`[model busy: ${event.reason}; retry ${event.attempt} of ${event.retries} in ${Math.ceil(event.delay_ms / 1000)} s]\n`);
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
  // "Always" allows this tool for the rest of the session. The server decides
  // whether it may (a tool that needs `execute` is asked about every time);
  // the button only follows `may_remember`.
  const choices = [["Deny", false, false], ["Allow", true, false]];
  if (request.may_remember) choices.push(["Allow for this session", true, true]);
  for (const [label, approve, always] of choices) {
    const button = el("button", approve ? "" : "ghost", label);
    button.type = "button";
    button.addEventListener("click", () => {
      send({ type: "approval", id: request.id, approve, always });
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
const files = {
  root: null,
  // Directory path ("" is the root) -> its listing, once loaded.
  dirs: new Map(),
  expanded: new Set([""]),
  // With several workspace folders: whether the active one is folded.
  collapsedActive: false,
  // Open files in tab order: {path, data, error}.
  open: [],
  current: null,
};

function sidePrefs() {
  return state.prefs.side || {};
}

function saveSidePrefs(change) {
  state.prefs.side = { ...sidePrefs(), ...change };
  savePrefs();
}

function sideView() {
  return $("plugins-panel").hidden ? "files" : "plugins";
}

function showSide(view) {
  if (view === "files" && !workspaceShown()) return;
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

// ---- Modes ----------------------------------------------------------------

function activeMode() {
  return state.modes.modes.find((mode) => mode.id === state.modes.active) || null;
}

// Whether the active mode shows the workspace (files, Git, the folder).
function workspaceShown() {
  return activeMode()?.workspace !== false;
}

// The mode whose API groups are offered: the active mode, or the parent of
// the active sub-mode.
function groupParent() {
  const active = activeMode();
  return active?.extends || active?.id || null;
}

function renderModes() {
  const select = $("mode");
  select.hidden = state.modes.modes.length === 0;
  const none = el("option", "", "No mode");
  none.value = "";
  // Sub-modes (API groups) are not listed here: they are the buttons below.
  const options = state.modes.modes
    .filter((mode) => !mode.extends)
    .map((mode) => {
      const option = el("option", "", mode.label);
      option.value = mode.id;
      option.title = mode.description || "";
      return option;
    });
  select.replaceChildren(none, ...options);
  select.value = groupParent() || "";
  select.title = activeMode()?.description || "Mode: the tools, instructions and panels to work with";
  renderApiGroups();
}

// The internal APIs as one choice above the message box: "Off" or one API
// (a group, or a sub-mode of the selected mode). Only one can be connected,
// and the line beside it says what the model can reach right now.
function renderApiGroups() {
  const box = $("api-groups");
  const parent = groupParent();
  const subModes = parent ? state.modes.modes.filter((mode) => mode.extends === parent) : [];
  // Instances no mode lists are offered in a mode with `groups`.
  const offered = activeMode()?.groups ? state.groups : [];
  $("api-bar").hidden = subModes.length === 0 && offered.length === 0;
  const connected = offered.find((group) => group.active)?.label
    || subModes.find((mode) => mode.id === state.modes.active)?.label
    || null;
  const choice = (label, on, title, run) => {
    const button = el("button", on ? "group on" : "group", label);
    button.type = "button";
    button.setAttribute("role", "radio");
    button.setAttribute("aria-checked", String(on));
    button.title = title;
    button.addEventListener("click", () => {
      if (!on) run();
    });
    return button;
  };
  const off = async () => {
    const group = offered.find((item) => item.active);
    if (group) await switchGroup(group.id, false);
    if (parent && state.modes.active !== parent) await switchMode(parent);
  };
  box.replaceChildren(
    choice("Off", connected === null, "Connect no internal API", off),
    ...offered.map((group) =>
      choice(group.label, group.active, `Connect the ${group.label} API (disconnects any other)`, () =>
        switchGroup(group.id, true),
      ),
    ),
    ...subModes.map((mode) =>
      choice(mode.label, mode.id === state.modes.active, mode.description || `Connect the ${mode.label} API`, () =>
        switchMode(mode.id),
      ),
    ),
  );
  const line = $("api-state");
  line.textContent = connected
    ? `Connected to ${connected}: the model can query this API.`
    : "No API connected: the model cannot reach any internal API.";
  line.classList.toggle("connected", connected !== null);
}

// A mode or group switch starts and stops plugins: show it in the plugin list.
function refreshPluginList() {
  if (!$("side").hidden && sideView() === "plugins") loadPlugins().catch(report);
}

// Applies the active mode to this window. The side panel's own preference
// is kept: a mode without the workspace only hides the files view.
function applyMode() {
  renderModes();
  refreshPluginList();
  const shown = workspaceShown();
  document.body.classList.toggle("no-workspace", !shown);
  if (!shown && !$("side").hidden && sideView() === "files") $("side").hidden = true;
  if (shown && $("side").hidden && sidePrefs().open) {
    showSide(sidePrefs().view === "plugins" ? "plugins" : "files");
  }
}

async function loadModes() {
  try {
    state.modes = await api("/modes");
  } catch {
    // Older servers have no modes.
    state.modes = { modes: [], active: null };
  }
  applyMode();
}

async function loadGroups() {
  try {
    state.groups = (await api("/groups")).groups;
  } catch {
    // Older servers have no groups.
    state.groups = [];
  }
  renderApiGroups();
  refreshPluginList();
}

async function switchGroup(id, on) {
  for (const button of $("api-groups").children) button.disabled = true;
  try {
    state.groups = (await api("/group", { method: "PUT", body: JSON.stringify({ id, on }) })).groups;
    refreshStatus();
  } catch (error) {
    report(error);
  } finally {
    renderApiGroups();
    refreshPluginList();
  }
}

async function switchMode(id) {
  $("mode").disabled = true;
  try {
    const reply = await api("/mode", {
      method: "PUT",
      body: JSON.stringify({ mode: id || null }),
    });
    state.modes = { modes: reply.modes, active: reply.active };
    applyMode();
    if (reply.problems?.length) showNotice(`Some plugins were not switched: ${reply.problems.join("; ")}`);
    refreshStatus();
  } catch (error) {
    report(error);
    renderModes();
  } finally {
    $("mode").disabled = false;
  }
}

// The header buttons open their view, or close the panel when it shows it.
function toggleSide(view) {
  if (!$("side").hidden && sideView() === view) hideSide();
  else showSide(view);
}

function treeError(error) {
  $("tree").replaceChildren(el("li", "note", error.message));
}

// The tree holds every folder of the workspace. A folder is "" for the
// active one (the sandbox root) and its path for the others; a directory or
// file is keyed by its path in the active folder and by folder and path in
// the others.
function dirKey(folder, path) {
  return folder ? `${folder}\u0000${path}` : path;
}

function splitKey(key) {
  const cut = key.indexOf("\u0000");
  return cut < 0 ? ["", key] : [key.slice(0, cut), key.slice(cut + 1)];
}

function folderQuery(folder) {
  return folder ? `&folder=${encodeURIComponent(folder)}` : "";
}

async function loadDir(path, folder = "") {
  const listing = await api(`/files?path=${encodeURIComponent(path)}${folderQuery(folder)}`);
  if (path === "" && !folder) files.root = listing.root;
  files.dirs.set(dirKey(folder, path), listing);
  renderTree();
}

// A turn may have changed files: reload the open folders and files.
async function reloadFiles() {
  if (!files.dirs.has("")) {
    if (git.loaded) await loadGit();
    return;
  }
  const keys = [...files.expanded].filter((key) => files.dirs.has(key));
  files.dirs.clear();
  for (const key of keys) {
    try {
      const [folder, path] = splitKey(key);
      await loadDir(path, folder);
    } catch {
      // Gone: shown closed.
      files.expanded.delete(key);
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
  files.collapsedActive = false;
  files.open = [];
  files.current = null;
  renderTreeHead();
  $("tree").replaceChildren();
  renderFile();
  git.loaded = false;
  git.collapsed.clear();
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

function treeNodes(path, depth, folder = "") {
  const listing = files.dirs.get(dirKey(folder, path));
  if (!listing) return [treeNote("Loading…", depth)];
  const nodes = listing.entries.map((entry) => {
    const key = dirKey(folder, entry.path);
    const item = el("li", entry.dir ? "dir" : "file");
    item.setAttribute("role", "treeitem");
    const button = el("button");
    button.type = "button";
    button.title = entry.path;
    button.dataset.path = entry.path;
    button.style.setProperty("--depth", depth);
    const open = entry.dir && files.expanded.has(key);
    if (entry.dir) item.setAttribute("aria-expanded", String(open));
    if (!entry.dir && key === files.current) button.classList.add("open-file");
    button.append(
      el("span", "caret", entry.dir ? (open ? "▾" : "▸") : ""),
      el("span", "icon-glyph", entry.dir ? "▣" : "▢"),
      el("span", "", entry.name),
    );
    button.addEventListener("click", () => {
      if (!entry.dir) {
        openFile(entry.path, folder).catch(report);
        return;
      }
      if (open) files.expanded.delete(key);
      else files.expanded.add(key);
      if (!open && !files.dirs.has(key)) loadDir(entry.path, folder).catch(report);
      renderTree();
    });
    item.append(button);
    if (open) {
      const children = el("ul");
      children.setAttribute("role", "group");
      children.append(...treeNodes(entry.path, depth + 1, folder));
      item.append(children);
    }
    return item;
  });
  if (!listing.entries.length) nodes.push(treeNote("Empty folder", depth));
  if (listing.truncated) nodes.push(treeNote("…more entries not shown", depth));
  return nodes;
}

// The folders of the workspace (`GET /api/workspace`), the active one marked.
function workspaceFolders() {
  return state.workspaceInfo?.folders || [];
}

function severalFolders() {
  return workspaceFolders().length > 1;
}

// The tree's folder parameter for a workspace folder.
function folderParam(folder) {
  return folder.active ? "" : folder.path;
}

function renderTreeHead() {
  const head = $("tree-root");
  const info = state.workspaceInfo;
  if (severalFolders()) {
    head.textContent = info.name || "Workspace";
    head.title = `${info.name ? `Workspace ${info.name}` : "Workspace"}: ${info.folders.length} folders`;
  } else {
    head.textContent = files.root ? folderName(files.root) : "Files";
    head.title = files.root || "";
  }
}

// A small action button of a folder row; a click does not fold the folder.
function folderAction(label, title, run) {
  const button = el("button", "row-action", label);
  button.type = "button";
  button.title = title;
  button.setAttribute("aria-label", title);
  button.addEventListener("click", (event) => {
    event.stopPropagation();
    run();
  });
  return button;
}

// One workspace folder as the top of its tree: the active folder is open
// until closed, the others closed until opened.
function folderNode(folder) {
  const param = folderParam(folder);
  const key = dirKey(param, "");
  const open = folder.active ? !files.collapsedActive : files.expanded.has(key);
  const item = el("li", folder.active ? "dir root-folder active" : "dir root-folder");
  item.setAttribute("role", "treeitem");
  item.setAttribute("aria-expanded", String(open));
  const row = el("div", "folder-row");
  const button = el("button");
  button.type = "button";
  button.title = folder.active ? `${folder.path}\nThe active folder: tools work here` : folder.path;
  button.dataset.folder = folder.path;
  button.append(el("span", "caret", open ? "▾" : "▸"), el("span", "folder-name", folder.name));
  if (folder.active) button.append(el("span", "tag", "active"));
  button.addEventListener("click", () => {
    if (folder.active) files.collapsedActive = open;
    else if (open) files.expanded.delete(key);
    else files.expanded.add(key);
    if (!open && !files.dirs.has(key)) loadDir("", param).catch(report);
    renderTree();
  });
  row.append(button);
  if (!folder.active) {
    row.append(folderAction("◉", `Make ${folder.name} the active folder (tools work there)`, () => activateFolder(folder.path)));
  }
  row.append(folderAction("✕", `Remove ${folder.name} from the workspace`, () => removeFolder(folder.path)));
  item.append(row);
  if (open) {
    const children = el("ul");
    children.setAttribute("role", "group");
    children.append(...treeNodes("", 1, param));
    item.append(children);
  }
  return item;
}

function renderTree() {
  renderTreeHead();
  const tree = $("tree");
  const scroll = tree.scrollTop;
  tree.replaceChildren(...(severalFolders() ? workspaceFolders().map(folderNode) : treeNodes("", 0)));
  tree.scrollTop = scroll;
}

// An open tab is a file or a file's diff: {key, kind, path, folder, label,
// data, error}. A file's key is its tree key, so the tree can mark it.
const TAB_SOURCES = {
  file: (tab) => `/files/content?path=${encodeURIComponent(tab.path)}${folderQuery(tab.folder)}`,
  diff: (tab) => `/git/diff?path=${encodeURIComponent(tab.path)}&repo=${encodeURIComponent(tab.repo || "")}`,
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

async function openTab(kind, path, label, extra = {}) {
  const key = kind === "file" ? dirKey(extra.folder || "", path) : `${kind}:${path}`;
  let file = files.open.find((open) => open.key === key);
  if (!file) {
    file = { key, kind, path, label, data: null, error: null, ...extra };
    files.open.push(file);
    await readFile(file);
  }
  files.current = key;
  if ($("side").hidden || sideView() !== "files") showSide("files");
  renderFile();
  renderTree();
  renderGit();
}

// `folder` is "" for the active folder, otherwise another workspace folder.
function openFile(path, folder = "") {
  return openTab("file", path, folderName(path), { folder });
}

// `repo` is the repository's folder in the workspace ("" for the root).
function openDiff(path, repo) {
  git.focus = repo || "";
  return openTab("diff", path, `${folderName(path)} (diff)`, { repo });
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
// `node` cut at its line breaks: one copy of `node` per line, each holding
// only the part of the content on that line, so highlighting that spans lines
// (a block comment) survives the cut.
function splitByLine(node) {
  if (node.nodeType === Node.TEXT_NODE) {
    return node.data.split("\n").map((part) => document.createTextNode(part));
  }
  const lines = [[]];
  for (const child of node.childNodes) {
    splitByLine(child).forEach((part, index) => {
      if (index > 0) lines.push([]);
      lines[lines.length - 1].push(part);
    });
  }
  return lines.map((children) => {
    const copy = node.cloneNode(false);
    copy.append(...children);
    return copy;
  });
}

function codeView(text, path) {
  const lines = (text.endsWith("\n") ? text.slice(0, -1) : text).split("\n");
  const highlighted = Highlight.render(lines.join("\n"), path);
  if (state.prefs.wrap_files) {
    // One row per line: a line number stays beside the first row of its line.
    const code = el("div", "code wrap");
    code.style.setProperty("--num-width", `${String(lines.length).length}ch`);
    code.append(
      ...splitByLine(highlighted).map((part, index) => {
        const row = el("div", "row");
        const text = el("span", "text");
        text.append(part);
        row.append(el("span", "num", String(index + 1)), text);
        return row;
      }),
    );
    return code;
  }
  const code = el("div", "code");
  const body = el("pre", "lines");
  body.append(highlighted);
  code.append(el("pre", "gutter", lines.map((_, index) => index + 1).join("\n")), body);
  return code;
}

// The wrap switches (files and activity) and the theme come from the
// preferences, so the browser and the desktop agree.
function applyWrap() {
  const files = Boolean(state.prefs.wrap_files);
  const activity = Boolean(state.prefs.wrap_activity);
  $("wrap-files").setAttribute("aria-pressed", String(files));
  $("wrap-activity").setAttribute("aria-pressed", String(activity));
  $("activity").classList.toggle("wrap", activity);
}

const THEMES = [
  ["system", "Follow the system"],
  ["light", "Light"],
  ["dark", "Dark"],
  ["sepia", "Sepia"],
  ["midnight", "Midnight"],
  ["contrast", "High contrast"],
];

function currentTheme() {
  return THEMES.some(([id]) => id === state.prefs.theme) ? state.prefs.theme : "system";
}

// `data-theme` picks a theme; without it the system decides (see app.css).
function applyTheme() {
  const theme = currentTheme();
  if (theme === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = theme;
}

function toggleWrap(key) {
  state.prefs[key] = !state.prefs[key];
  savePrefs();
  applyWrap();
  if (key === "wrap_files") renderFile();
}

function fileTab(file) {
  const current = file.key === files.current;
  const tab = el("div", current ? `file-tab current ${file.kind}` : `file-tab ${file.kind}`);
  tab.setAttribute("role", "tab");
  tab.setAttribute("aria-selected", String(current));
  tab.title = file.kind === "file" ? (file.folder ? `${file.folder}: ${file.path}` : file.path) : `${file.label}: ${file.path}`;
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
  // The status line shows the branch of the open file's repository.
  renderGitStatus();
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

// The Git state under the file tree, read-only: per repository (the root's,
// and the ones in folders below it), the branch and the changed files,
// grouped by folder. A repository is `{path, branch, changes, truncated}` or
// `{path, error}`; `path` is its folder ("" for the root). Paths are
// untrusted text.
// `focus` is the repository the user last worked with ("" for the root).
const git = { loaded: false, loading: null, repos: [], error: null, signature: null, collapsed: new Set(), focus: null };

const STATUS_NAMES = {
  M: "modified", A: "added", D: "deleted", R: "renamed", C: "copied", T: "type changed", U: "untracked", "!": "conflicted",
};

async function loadGit() {
  if (git.loading) return git.loading;
  git.loading = (async () => {
    // The other folders' branches come with the workspace.
    if (severalFolders()) await loadWorkspace();
    try {
      const listing = await api("/git/changes");
      // Not a repository (or no Git): a state, not a failure.
      if (listing.error) throw new Error(listing.error);
      Object.assign(git, { repos: listing.repos, error: null });
    } catch (error) {
      Object.assign(git, { repos: [], error: error.message });
    } finally {
      git.loaded = true;
      git.loading = null;
    }
    renderGit();
  })();
  return git.loading;
}

function branchLabel(branch) {
  return branch && branch !== "(detached)" ? `⎇ ${branch}` : branch ? "⎇ detached" : "";
}

function changeItem(change, nested, repo) {
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
  button.addEventListener("click", () => (change.status === "U" ? openFile(change.path) : openDiff(change.path, repo)).catch(report));
  item.append(button);
  return item;
}

// One repository's changed files: one row per folder (relative to the
// repository), then its files; files at the repository's root come last.
function changeRows(repo) {
  if (repo.error) return [el("li", "note", repo.error)];
  const prefix = repo.path ? `${repo.path}/` : "";
  const folders = new Map();
  for (const change of repo.changes) {
    const inner = change.path.startsWith(prefix) ? change.path.slice(prefix.length) : change.path;
    const cut = inner.lastIndexOf("/");
    const folder = cut < 0 ? "" : inner.slice(0, cut);
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
    for (const change of folders.get(folder)) rows.push(changeItem(change, !!folder, repo.path));
  }
  if (!rows.length) rows.push(el("li", "note", "No changes"));
  if (repo.truncated) rows.push(el("li", "note", "…more changes not shown"));
  return rows;
}

// One collapsible section per repository. With one repository the section is
// "Changes" and its branch sits in the pane header; with several, each
// section is named after its folder and carries its own branch control.
function repoSection(repo, several) {
  const section = el("details");
  section.open = !git.collapsed.has(repo.path);
  section.addEventListener("toggle", () => {
    if (section.open) git.collapsed.delete(repo.path);
    else git.collapsed.add(repo.path);
  });
  const summary = el("summary");
  if (several) {
    const name = el("span", "repo-name", repo.path ? folderName(repo.path) : files.root ? folderName(files.root) : "Workspace");
    name.title = repo.path || "The workspace folder";
    summary.append(name);
    const label = branchLabel(repo.branch);
    if (label) {
      const branch = el("button", "git-branch", label);
      branch.type = "button";
      branch.title = "Switch branch";
      branch.setAttribute("aria-haspopup", "listbox");
      branch.addEventListener("click", (event) => {
        // A click in the summary would also fold the section.
        event.preventDefault();
        event.stopPropagation();
        openBranchMenu(branch, repo.path).catch(report);
      });
      summary.append(branch);
    }
  } else {
    summary.append("Changes");
  }
  summary.append(el("span", "count", repo.error ? "" : String(repo.changes.length)));
  const list = el("ul", "tree changes");
  list.setAttribute("aria-label", repo.path ? `Changed files in ${repo.path}` : "Changed files");
  list.append(...changeRows(repo));
  section.append(summary, list);
  return section;
}

// Another workspace folder: Git runs only in the active folder (through the
// sandbox), so it shows its repositories' branches and offers to make the
// folder active.
function otherFolderSection(folder) {
  const key = `folder:${folder.path}`;
  const section = el("details", "other-folder");
  section.open = !git.collapsed.has(key);
  section.addEventListener("toggle", () => {
    if (section.open) git.collapsed.delete(key);
    else git.collapsed.add(key);
  });
  const summary = el("summary");
  const name = el("span", "repo-name", folder.name);
  name.title = folder.path;
  summary.append(name);
  const list = el("ul", "tree changes");
  list.setAttribute("aria-label", `Repositories in ${folder.name}`);
  for (const repo of folder.repos || []) {
    const row = el("li", "note", `${branchLabel(repo.branch)}${repo.path ? ` · ${repo.path}` : ""}`);
    row.title = repo.path ? `${folder.path}: ${repo.path}` : folder.path;
    list.append(row);
  }
  if (!folder.repos?.length) list.append(el("li", "note", "No Git repository"));
  const item = el("li");
  const activate = el("button", "activate", "Make active to see its changes");
  activate.type = "button";
  activate.title = `Make ${folder.name} the active folder: tools and Git work there`;
  activate.addEventListener("click", () => activateFolder(folder.path));
  item.append(activate);
  list.append(item);
  section.append(summary, list);
  return section;
}

function renderGit() {
  const others = workspaceFolders().filter((folder) => !folder.active);
  const several = git.repos.length > 1 || others.length > 0;
  const header = $("git-branch");
  const only = !several ? git.repos[0] : null;
  header.textContent = only ? branchLabel(only.branch) : "";
  header.dataset.repo = only ? only.path : "";
  const body = $("git-body");
  const scroll = body.scrollTop;
  const sections = others.map(otherFolderSection);
  if (!git.loaded || git.error) {
    const list = el("ul", "tree changes");
    list.append(el("li", "note", git.loaded ? git.error : "Loading…"));
    body.replaceChildren(list, ...sections);
    return;
  }
  body.replaceChildren(...git.repos.map((repo) => repoSection(repo, several)), ...sections);
  body.scrollTop = scroll;
}

// The branch menu of one repository (`repo` is its folder, "" for the root),
// opened from the status line, the source control header or a repository's
// section. Switching runs `git switch` in that repository, then everything
// that shows the work tree is read again: status line, file tree, open tabs,
// changes.
function closeBranchMenu() {
  $("branch-menu").hidden = true;
}

async function openBranchMenu(anchor, repo = "") {
  git.focus = repo;
  renderGitStatus();
  const menu = $("branch-menu");
  if (!menu.hidden && menu.anchor === anchor) {
    closeBranchMenu();
    return;
  }
  menu.anchor = anchor;
  menu.replaceChildren(el("div", "note", "Loading…"));
  menu.hidden = false;
  const box = anchor.getBoundingClientRect();
  const left = Math.max(8, Math.min(box.left, window.innerWidth - menu.offsetWidth - 8));
  menu.style.left = `${left}px`;
  // Below the control when it sits in the top half, above it otherwise.
  const above = box.top > window.innerHeight / 2;
  menu.style.top = above ? "" : `${box.bottom + 4}px`;
  menu.style.bottom = above ? `${window.innerHeight - box.top + 4}px` : "";
  let listing;
  try {
    listing = await api(`/git/branches?repo=${encodeURIComponent(repo)}`);
  } catch (error) {
    menu.replaceChildren(el("div", "note", error.message));
    return;
  }
  const rows = listing.branches.map((branch) => {
    const button = el("button");
    button.type = "button";
    button.setAttribute("role", "option");
    const current = branch.name === listing.current;
    button.classList.toggle("current", current);
    button.setAttribute("aria-selected", String(current));
    button.title = branch.remote ? `Create a local branch tracking ${branch.name}` : `Switch to ${branch.name}`;
    button.append(el("span", "menu-name", branch.name), el("span", "menu-tag", current ? "✓" : branch.remote ? "remote" : ""));
    button.addEventListener("click", () => switchBranch(branch.name, current, repo).catch(report));
    return button;
  });
  if (!rows.length) rows.push(el("div", "note", "No branches"));
  menu.replaceChildren(...rows);
  (menu.querySelector(".current") || rows[0])?.focus?.();
}

async function switchBranch(name, current, repo) {
  closeBranchMenu();
  if (current) return;
  if (state.running) {
    showNotice("A turn is running: switching branch now would change files under it. Cancel it or wait.");
    return;
  }
  await api("/git/checkout", { method: "POST", body: JSON.stringify({ branch: name, repo }) });
  showInfo(repo ? `Switched ${repo} to branch ${name}.` : `Switched to branch ${name}.`);
  refreshStatus();
  await reloadFiles();
}

// The status line's Git summary changed (every 30 seconds it is read): the
// list is probably stale.
function gitSignature(state) {
  const signature = state ? JSON.stringify(state) : "";
  if (signature !== git.signature) {
    const known = git.signature !== null;
    git.signature = signature;
    if (known && git.loaded && !$("side").hidden) loadGit();
  }
}

// A draggable divider, for every divider that needs no code of its own.
// `size(event)` turns the pointer into a size (pixels, or a share between 0
// and 1), `bounds()` limits it while dragging, `apply(size)` shows it and
// `current()` reads the size shown. The size is kept in the server
// preferences as `layout[name]` (or where `stored` says: `get()` and
// `set(size)`), and a saved size is shown again when `valid(size)`. The arrow
// keys `keys` (less, more) move it by `step`.
function setupSplit({ handle, name, keys, step, size, bounds, apply, current, valid = (value) => value > 0, stored }) {
  const layout = () => state.prefs.layout || {};
  const clamp = (value) => {
    const [low, high] = bounds();
    return Math.round(Math.min(Math.max(value, low), Math.max(low, high)) * 1000) / 1000;
  };
  const keep = (value) => {
    if (stored) {
      stored.set(value);
      return;
    }
    state.prefs.layout = { ...layout(), [name]: value };
    savePrefs();
  };
  // The CSS limits (min and max sizes) hold a saved size that no longer fits.
  const saved = Number(stored ? stored.get() : layout()[name]);
  if (valid(saved)) apply(saved);
  handle.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    handle.classList.add("dragging");
    let value = null;
    const move = (moved) => {
      value = clamp(size(moved));
      apply(value);
    };
    const done = () => {
      handle.classList.remove("dragging");
      handle.removeEventListener("pointermove", move);
      if (value !== null) keep(value);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", done, { once: true });
    handle.addEventListener("pointercancel", done, { once: true });
  });
  handle.addEventListener("keydown", (event) => {
    const direction = keys.indexOf(event.key);
    if (direction < 0) return;
    event.preventDefault();
    const next = clamp(current() + (direction === 0 ? -step : step));
    apply(next);
    keep(next);
  });
}

// Every divider: the right-hand panel, the file tree and source control, the
// session list, the file tree and the file view.
function setupLayout() {
  // The side panel's width and the tree/Git split keep their place in `side`.
  setupSplit({
    handle: $("side-resizer"),
    name: "side_width",
    keys: ["ArrowRight", "ArrowLeft"],
    step: 20,
    size: (event) => window.innerWidth - event.clientX,
    bounds: () => [320, window.innerWidth * 0.75],
    apply: (value) => document.documentElement.style.setProperty("--side-width", `${value}px`),
    current: () => $("side").getBoundingClientRect().width,
    stored: { get: () => sidePrefs().width, set: (value) => saveSidePrefs({ width: Math.round(value) }) },
  });
  const treePane = $("tree-pane");
  setupSplit({
    handle: $("pane-splitter"),
    name: "tree_ratio",
    keys: ["ArrowUp", "ArrowDown"],
    step: 0.05,
    size: (event) => {
      const box = treePane.getBoundingClientRect();
      return (event.clientY - box.top) / box.height;
    },
    bounds: () => [0.1, 0.9],
    apply: (value) => treePane.style.setProperty("--tree-ratio", Number(value).toFixed(3)),
    current: () => Number(getComputedStyle(treePane).getPropertyValue("--tree-ratio")) || 0.6,
    valid: (value) => value > 0 && value < 1,
    stored: { get: () => sidePrefs().tree_ratio, set: (value) => saveSidePrefs({ tree_ratio: value }) },
  });
  const sessions = $("sessions-splitter");
  setupSplit({
    handle: sessions,
    name: "sessions_width",
    keys: ["ArrowLeft", "ArrowRight"],
    step: 20,
    size: (event) => event.clientX - sessions.previousElementSibling.getBoundingClientRect().left,
    bounds: () => [160, Math.min(520, window.innerWidth * 0.4)],
    apply: (value) => document.documentElement.style.setProperty("--sessions-width", `${value}px`),
    current: () => sessions.previousElementSibling.getBoundingClientRect().width,
  });
  const tree = $("tree-pane");
  setupSplit({
    handle: $("tree-splitter"),
    name: "tree_width",
    keys: ["ArrowLeft", "ArrowRight"],
    step: 20,
    size: (event) => event.clientX - tree.getBoundingClientRect().left,
    bounds: () => [120, $("files-view").getBoundingClientRect().width - 160],
    apply: (value) => tree.style.setProperty("--tree-width", `${value}px`),
    current: () => tree.getBoundingClientRect().width,
  });
  // The file view above and the activity log below share what is left.
  const shown = () => {
    const view = $("file-view").getBoundingClientRect();
    return { top: view.top, total: view.height + $("activity").getBoundingClientRect().height };
  };
  setupSplit({
    handle: $("view-splitter"),
    name: "view_ratio",
    keys: ["ArrowUp", "ArrowDown"],
    step: 0.05,
    size: (event) => {
      const { top, total } = shown();
      return total > 0 ? (event.clientY - top) / total : 0.75;
    },
    bounds: () => [0.15, 0.9],
    apply: (value) => $("viewer-pane").style.setProperty("--view-ratio", String(value)),
    current: () => {
      const { total } = shown();
      return total > 0 ? $("file-view").getBoundingClientRect().height / total : 0.75;
    },
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
    case "model_retrying":
      logActivity(`↻ model busy, retry ${event.attempt} of ${event.retries} in ${Math.ceil(event.delay_ms / 1000)} s`, "warn");
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
    // Shown only while the link to the local RAH process is down.
    $("connection").textContent = "connected";
    $("connection").hidden = true;
    // Keep other notices, such as a failed startup request.
    if ($("notice-text").textContent === NOT_CONNECTED) showNotice("");
    // A switch made while this window was not connected (such as the mode
    // entered at start) sent no notice here.
    loadModes();
    loadGroups();
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
    } else if (data.type === "groups") {
      loadGroups();
    } else if (data.type === "host_notice") {
      // A plugin skipped something quietly that the user can act on.
      showTransientNotice(`${data.plugin}: ${data.message}`, 20000);
    } else if (data.type === "mode") {
      // Another window (or this one) switched the mode.
      if (data.active !== state.modes.active) {
        state.modes.active = data.active;
        applyMode();
        refreshStatus();
      }
    }
    else if (data.type === "error") {
      showNotice(data.message);
      setRunning(false);
    }
  });
  socket.addEventListener("close", () => {
    $("connection").textContent = "disconnected";
    $("connection").hidden = false;
    setRunning(false);
    setTimeout(connect, state.retry);
    state.retry = Math.min(state.retry * 2, 10000);
  });
}

// ---- Plugins --------------------------------------------------------------

// ---- Plugin management ----------------------------------------------------

const editor = { mode: null, target: null };
// Example configurations by kind, from `/api/catalog`.
let configExamples = {};
// Help for configuration keys by kind (`[{key, help}]`), from `/api/catalog`.
let configHelp = {};

function isEmptyConfig(config) {
  return config == null || (typeof config === "object" && !Array.isArray(config) && Object.keys(config).length === 0);
}

function editorKind() {
  return editor.mode === "configure" ? editor.target?.kind : $("editor-kind").value;
}

// Example values such as "<model id>" stand for something only the user
// knows; a configuration still holding one is not saved.
function placeholderValues(config) {
  return JSON.stringify(config ?? null).match(/"<[^"<>]*>"/g) || [];
}

// Shows the kind's example configuration in the empty editor, as a
// reference only: an empty editor still means the defaults.
function showConfigExample() {
  const example = configExamples[editorKind()];
  $("editor-example").hidden = example === undefined;
  const textarea = $("editor-config");
  textarea.placeholder = example === undefined
    ? "{}"
    : `Example (not applied; leave empty for the defaults):\n${JSON.stringify(example, null, 2)}`;
  const lines = Math.max(textarea.value.split("\n").length, textarea.placeholder.split("\n").length);
  textarea.rows = Math.min(18, Math.max(6, lines));
  showConfigHelp();
}

// Lists what each configuration key of the kind does, when the
// distribution describes them.
function showConfigHelp() {
  const keys = configHelp[editorKind()] || [];
  $("editor-help").hidden = keys.length === 0;
  $("editor-help-list").replaceChildren(...keys.flatMap(({ key, help }) => [
    el("dt", "", key),
    el("dd", "", help),
  ]));
}

// Copies the example into the editor to start from.
function fillConfigExample() {
  const example = configExamples[editorKind()];
  const textarea = $("editor-config");
  if (example === undefined) return;
  if (textarea.value.trim() && !window.confirm("Replace the current configuration with the example?")) return;
  textarea.value = JSON.stringify(example, null, 2);
  showConfigExample();
  const missing = placeholderValues(example);
  showNotice(missing.length ? `Replace ${missing.join(", ")} before saving.` : "");
  textarea.focus();
}

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
  loadGroups();
  return result;
}

// An API group (tools-http) needs a name; its button shows it without "http-".
function showIdHint() {
  const group = $("editor-kind").value === "tools-http";
  $("editor-id").placeholder = group ? "required, e.g. http-ddt (the button reads \"ddt\")" : "defaults to the kind";
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
  const config = mode === "add" ? null : plugin?.config;
  $("editor-config").value = isEmptyConfig(config) ? "" : JSON.stringify(config, null, 2);
  showConfigExample();
  showIdHint();
  $("editor").hidden = false;
  $("editor").scrollIntoView({ block: "nearest" });
  $("editor-id").focus();
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
  const missing = placeholderValues(config);
  if (missing.length) {
    showNotice(`Replace ${missing.join(", ")} before saving.`);
    return;
  }
  const kind = $("editor-kind").value;
  const id = $("editor-id").value.trim() || null;
  if (editor.mode === "add" && kind === "tools-http" && !id) {
    showNotice("Name this API group (for example http-ddt) so its button can be told apart.");
    $("editor-id").focus();
    return;
  }
  const request = { kind, id, config };
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
    configExamples = catalog.examples || {};
    configHelp = catalog.help || {};
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
  node.className = `item${className ? ` ${className}` : ""}${id === "status-root" ? " path link" : id === "status-git" ? " link" : ""}`;
  node.hidden = !text;
}

// The repository the user works in, among the active folder's `repos` (as
// the status reports them): the one holding the open file or diff, else the
// one last used in source control, else the root's (or the first).
function currentRepo(repos) {
  const tab = files.open.find((file) => file.key === files.current);
  if (tab && !tab.folder) {
    const holding = tab.kind === "diff"
      ? repos.find((repo) => repo.path === (tab.repo || ""))
      : repos
          .filter((repo) => !repo.path || tab.path === repo.path || tab.path.startsWith(`${repo.path}/`))
          .sort((a, b) => b.path.length - a.path.length)[0];
    if (holding) return holding;
  }
  return repos.find((repo) => repo.path === git.focus) || repos[0];
}

// `[label, title, repo]` of the status line's Git item: the current
// repository's branch, its changes and how it stands to its upstream.
function gitText(status) {
  if (!status) return ["", "", null];
  if (status.error) return ["no git repository", status.error, null];
  const repos = status.repos || [];
  const repo = currentRepo(repos);
  if (!repo) return ["", "", null];
  const parts = [`⎇ ${repo.detached ? "detached HEAD" : repo.branch}`];
  const named = repos.length > 1 || severalFolders();
  const where = repo.path ? folderName(repo.path) : files.root ? folderName(files.root) : "";
  if (named && where) parts.push(`· ${where}`);
  const dirty = repo.changed + repo.untracked;
  if (dirty) parts.push(`● ${dirty}`);
  if (repo.conflicts) parts.push(`⚠ ${repo.conflicts} conflicts`);
  if (repo.ahead) parts.push(`↑${repo.ahead}`);
  if (repo.behind) parts.push(`↓${repo.behind}`);
  const title = [
    repos.length > 1 ? `Repository ${repo.path || where || "at the root"} (one of ${repos.length})` : null,
    repo.detached ? "Detached HEAD" : `Branch ${repo.branch}`,
    repo.upstream ? `tracking ${repo.upstream} (ahead ${repo.ahead}, behind ${repo.behind})` : "no upstream",
    `${repo.changed} changed, ${repo.untracked} untracked, ${repo.conflicts} conflicts`,
    "Click to switch branch",
  ].filter(Boolean).join("\n");
  return [parts.join(" "), title, repo];
}

// The status line's Git item, from the latest status.
function renderGitStatus() {
  if (!$("status-git")) return;
  const status = state.gitStatus;
  const [label, title, repo] = gitText(status);
  const dirty = repo && (repo.changed + repo.untracked > 0 || repo.conflicts);
  statusItem("status-git", label, title, dirty ? "dirty" : "");
  $("status-git").dataset.repo = repo ? repo.path : "";
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
  // Another window may have changed the folders or the name.
  const info = state.workspaceInfo;
  if (moved || !info || (workspace.folders ?? 1) !== info.folders.length || (workspace.name || null) !== (info.name || null)) {
    loadWorkspace().catch(() => {});
  }
  const several = (workspace.folders || 0) > 1;
  const named = workspace.name ? `${workspace.name}: ` : several ? `${workspace.folders} folders: ` : "";
  statusItem(
    "status-root",
    workspace.root ? `${named}${workspace.root}` : "Choose a workspace…",
    workspace.root
      ? `${several || workspace.name ? `Workspace ${workspace.name || ""} (${workspace.folders} folders)\nActive folder (tools work here): ` : "Workspace: "}${workspace.root}\nClick to change`
      : "Choose the folder tools work in",
  );
  gitSignature(workspace.git);
  state.gitStatus = workspace.git || null;
  renderGitStatus();
  const [model, modelTitle] = providerText("model", status.model);
  // The server behind the model may be down while RAH itself is fine.
  const down = status.model?.health?.state === "unavailable";
  const reason = status.model?.health?.reason || "unreachable";
  statusItem(
    "status-model",
    down ? `${model} ✕ ${reason}` : model,
    down ? `${modelTitle}\nNot reachable now: ${reason}` : modelTitle,
    status.model && !down ? "" : "bad",
  );
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

// Recent entries are a folder (its path) or a workspace `{name, folders,
// root}` of several folders or with a name.
function recentWorkspaces() {
  const list = state.prefs.recent_workspaces;
  if (!Array.isArray(list)) return [];
  return list.filter((item) => typeof item === "string" || (item && Array.isArray(item.folders) && item.folders.length));
}

function sameRecent(a, b) {
  if (typeof a === "string" || typeof b === "string") return typeof a === typeof b && sameFolder(a, b);
  return (a.name || null) === (b.name || null)
    && a.folders.length === b.folders.length
    && a.folders.every((folder) => b.folders.some((other) => sameFolder(folder, other)));
}

// Records the workspace as the server describes it (`GET /api/workspace`).
function rememberWorkspace(info) {
  if (!info?.root) return;
  const folders = info.folders.map((folder) => folder.path);
  const entry = folders.length > 1 || info.name
    ? { ...(info.name ? { name: info.name } : {}), folders, root: info.root }
    : info.root;
  state.prefs.recent_workspaces = [entry, ...recentWorkspaces().filter((item) => !sameRecent(item, entry))].slice(0, 8);
  savePrefs();
}

async function loadWorkspace() {
  try {
    state.workspaceInfo = await api("/workspace");
  } catch {
    // Older servers have no workspace of several folders.
    state.workspaceInfo = null;
  }
  renderTree();
  renderGit();
  renderGitStatus();
  return state.workspaceInfo;
}

// `PUT /api/workspace`: `root` becomes the active folder and, with
// `folders`, the workspace holds those folders under `name`. When the
// active folder moves, the plugins restart in it and everything that shows
// the workspace starts over. Returns false when it failed (the error is
// shown by `fail`).
async function changeWorkspace(body, fail = report) {
  const moves = !sameFolder(body.root, state.workspace);
  if (moves && state.running) {
    fail(new Error("Wait for the running turn to finish or cancel it first."));
    return false;
  }
  let result;
  try {
    result = await api("/workspace", { method: "PUT", body: JSON.stringify(body) });
  } catch (error) {
    fail(error);
    return false;
  }
  state.workspaceInfo = result.workspace;
  rememberWorkspace(result.workspace);
  const failed = result.plugins.filter((plugin) => plugin.state !== "active");
  if (failed.length) {
    fail(new Error(`Switched, but not every plugin started: ${failed.map((p) => `${p.id} (${p.state})`).join(", ")}`));
  }
  if (moves) {
    await loadPlugins().catch(report);
    await loadStatus().catch(report);
    // The open session belongs to the folder just left; close it.
    const info = currentInfo();
    if (info?.workspace && !sameFolder(info.workspace, result.root)) closeSession();
    await loadSessions().catch(report);
  } else {
    refreshStatus();
  }
  renderTree();
  renderGit();
  return failed.length === 0;
}

function currentFolders() {
  return workspaceFolders().map((folder) => folder.path);
}

function activateFolder(path) {
  return changeWorkspace({ root: path });
}

function removeFolder(path) {
  const rest = currentFolders().filter((folder) => !sameFolder(folder, path));
  if (!rest.length) {
    showTransientNotice("A workspace keeps at least one folder: open another folder instead.");
    return Promise.resolve(false);
  }
  const root = sameFolder(path, state.workspace) ? rest[0] : state.workspace;
  for (const key of [...files.expanded]) {
    if (splitKey(key)[0] && sameFolder(splitKey(key)[0], path)) files.expanded.delete(key);
  }
  return changeWorkspace({ root, folders: rest, name: state.workspaceInfo?.name || null });
}

function openRecent(entry) {
  const root = entry.root && entry.folders.some((folder) => sameFolder(folder, entry.root)) ? entry.root : entry.folders[0];
  return changeWorkspace({ root, folders: entry.folders, name: entry.name || null }, (error) => workspaceError(error.message));
}

function recentButton(entry) {
  if (typeof entry === "string") return folderButton(entry, entry, false, () => browse(entry));
  const label = `${entry.name || folderName(entry.folders[0])} (${entry.folders.length} ${entry.folders.length === 1 ? "folder" : "folders"})`;
  const item = folderButton(label, entry.folders.join("\n"), false, async () => {
    if (await openRecent(entry)) $("workspace-dialog").close();
  });
  item.querySelector("button").prepend(el("span", "kind", "⧉"));
  return item;
}

// The current workspace in the dialog: its name and folders.
function renderDialogWorkspace() {
  const info = state.workspaceInfo;
  $("ws-current-block").hidden = !info?.folders?.length;
  if (!info) return;
  $("ws-name").value = info.name || "";
  $("ws-folders").replaceChildren(
    ...info.folders.map((folder) => {
      const item = el("li");
      const button = el("button");
      button.type = "button";
      button.title = folder.path;
      button.append(el("span", "name", folder.path));
      if (folder.active) button.append(el("span", "tag", "active"));
      button.addEventListener("click", () => browse(folder.path));
      item.append(button);
      return item;
    }),
  );
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
  $("ws-recent").replaceChildren(...recent.map(recentButton));
  await loadWorkspace();
  renderDialogWorkspace();
  $("ws-add").hidden = !state.workspaceInfo?.folders?.length;
  $("workspace-dialog").showModal();
  await browse(state.workspace);
}

// The dialog's buttons: open the folder alone (a new workspace, as VS
// Code's Open Folder), add it to the workspace, or rename the workspace.
async function pickerChange(button, body) {
  button.disabled = true;
  try {
    if (await changeWorkspace(body, (error) => workspaceError(error.message))) $("workspace-dialog").close();
  } finally {
    button.disabled = false;
  }
}

function useWorkspace() {
  const path = $("ws-path").value.trim();
  if (!path) return Promise.resolve();
  return pickerChange($("ws-use"), { root: path, folders: [path] });
}

function addToWorkspace() {
  const path = $("ws-path").value.trim();
  if (!path || !state.workspace) return Promise.resolve();
  return pickerChange($("ws-add"), {
    root: state.workspace,
    folders: [...currentFolders(), path],
    name: state.workspaceInfo?.name || null,
  });
}

function saveWorkspaceName() {
  if (!state.workspace) return Promise.resolve();
  return pickerChange($("ws-save-name"), {
    root: state.workspace,
    folders: currentFolders(),
    name: $("ws-name").value.trim() || null,
  });
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
  try {
    state.prefs = { ...state.prefs, ...(await api("/preferences")) };
  } catch {
    // Older servers have no preferences; the defaults apply.
  }
  applyTheme();
  $("session-search").addEventListener("input", renderSessions);
  $("status-root").addEventListener("click", () => openWorkspacePicker().catch(report));
  $("git-branch").addEventListener("click", (event) => {
    event.stopPropagation();
    openBranchMenu($("git-branch"), $("git-branch").dataset.repo || "").catch(report);
  });
  $("status-git").addEventListener("click", (event) => {
    event.stopPropagation();
    // The branches of the repository the item shows.
    openBranchMenu($("status-git"), $("status-git").dataset.repo || "").catch(report);
  });
  document.addEventListener("click", (event) => {
    if (!$("branch-menu").contains(event.target)) closeBranchMenu();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !$("branch-menu").hidden) closeBranchMenu();
  });
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
  $("ws-add").addEventListener("click", () => addToWorkspace());
  $("ws-save-name").addEventListener("click", () => saveWorkspaceName());
  $("ws-name").addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      saveWorkspaceName();
    }
  });
  $("tree-add").addEventListener("click", () => openWorkspacePicker().catch(report));
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
  $("notice-close").addEventListener("click", () => showNotice(""));
  $("tree-refresh").addEventListener("click", () => reloadFiles().catch(report));
  $("close-all").addEventListener("click", closeAllFiles);
  $("wrap-files").addEventListener("click", () => toggleWrap("wrap_files"));
  $("wrap-activity").addEventListener("click", () => toggleWrap("wrap_activity"));
  applyWrap();
  $("activity-clear").addEventListener("click", () => $("activity").replaceChildren());
  setupLayout();
  $("git-refresh").addEventListener("click", () => loadGit());
  renderFile();
  renderGit();
  $("refresh-plugins").addEventListener("click", () => loadPlugins().catch(report));
  $("add-plugin").addEventListener("click", () => openEditor("add", null));
  $("editor").addEventListener("submit", saveEditor);
  $("editor-kind").addEventListener("change", () => {
    showConfigExample();
    showIdHint();
  });
  $("editor-example").addEventListener("click", fillConfigExample);
  $("editor-cancel").addEventListener("click", () => {
    $("editor").hidden = true;
  });
  setInterval(() => {
    if (!document.hidden) refreshStatus();
  }, 30000);
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) refreshStatus();
  });
  $("mode").addEventListener("change", () => switchMode($("mode").value));
  await loadModes();
  await loadGroups();
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
