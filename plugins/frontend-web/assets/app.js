// RAH Web UI. Model and tool output is untrusted: it is only ever inserted
// as text (textContent), never as HTML.
"use strict";

const $ = (id) => document.getElementById(id);

// The served title names the version, such as "RAH 0.45.0".
const PAGE_TITLE = document.title;

const state = {
  token: null,
  socket: null,
  sessions: [],
  current: null,
  running: false,
  // Preferences kept by the server (`/api/preferences`): display, the
  // folders opened in the session list, recent workspaces, the side panel.
  prefs: { raw_markdown: false },
  // `GET /api/groups`: the API groups (tools-http instances): `{id, label, active}`.
  groups: [],
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

let historySearchTimer;
let historySearchVersion = 0;
let historySearchController;
let historySearchHits = new Map();
let historySearchQuery = "";
let historySearchPending = false;
let historySearchError = "";

function searchSessions() {
  clearTimeout(historySearchTimer);
  historySearchController?.abort();
  historySearchController = new AbortController();
  const signal = historySearchController.signal;
  const version = ++historySearchVersion;
  const query = $("session-search").value.trim().toLowerCase();
  historySearchQuery = query;
  historySearchHits = new Map();
  historySearchPending = !!query;
  historySearchError = "";
  renderSessions();
  if (!query) return;
  historySearchTimer = setTimeout(async () => {
    try {
      const hits = await api(`/sessions/search?q=${encodeURIComponent(query)}`, { signal });
      if (version !== historySearchVersion) return;
      historySearchHits = new Map(hits.map((hit) => [hit.id, hit]));
    } catch (error) {
      if (version !== historySearchVersion) return;
      historySearchError = `History search failed: ${error.message}`;
    }
    if (version !== historySearchVersion) return;
    historySearchPending = false;
    renderSessions();
  }, 250);
}

function renderSessions() {
  const query = $("session-search").value.trim().toLowerCase();
  const list = $("sessions");
  list.replaceChildren();
  const visible = state.sessions.filter((info) => {
    const haystack = `${info.title || ""} ${info.preview || ""} ${info.id} ${info.workspace || ""}`.toLowerCase();
    return !query || haystack.includes(query) || (query === historySearchQuery && historySearchHits.has(info.id));
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
  if (query && historySearchPending) list.append(el("li", "group", "Searching message history..."));
  if (query && historySearchError) list.append(el("li", "group", historySearchError));
  if (query && historySearchHits.size >= 100) list.append(el("li", "group", "Showing up to 100 history matches. Refine your search for more."));
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
  const hit = $("session-search").value.trim().toLowerCase() === historySearchQuery ? historySearchHits.get(info.id) : null;
  if (hit?.snippet) item.append(el("small", "search-snippet", hit.snippet));
  item.addEventListener("click", async () => {
    try {
      await openSession(info.id);
      if (hit?.seq && state.current === info.id) {
        const entry = $("transcript").querySelector(`[data-seq="${Number(hit.seq)}"]`);
        entry?.scrollIntoView({ block: "center" });
        entry?.animate([{ outline: "2px solid currentColor" }, { outline: "2px solid transparent" }], 2000);
      }
    } catch (error) { report(error); }
  });
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
  refreshViews();
  $("transcript").replaceChildren();
  renderMessageNavigation();
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
  if (state.current !== id) {
    messageNavigationEntries = [];
    $("message-anchors").replaceChildren();
    $("message-nav").hidden = true;
  }
  state.current = id;
  renderUsage();
  // Views are rendered for the open session.
  refreshViews();
  setRunning(false);
  await ensureWorkspace(state.sessions.find((info) => info.id === id));
  await loadSessions();
  await loadTranscript();
  loadChanges().catch(report);
  $("input").disabled = false;
  $("send").disabled = false;
  $("input").focus();
}

// ---- Undo (ADR 0060) ------------------------------------------------------

// Shows the Undo button when the session has file changes to take back.
async function loadChanges() {
  const session = state.current;
  const button = $("session-undo");
  if (!session) {
    button.hidden = true;
    return;
  }
  const { available, changes } = await api(`/sessions/${session}/changes`);
  if (session !== state.current) return;
  const newest = changes[0];
  button.hidden = !available || !newest;
  button.textContent = changes.length > 1 ? `Undo (${changes.length})` : "Undo";
  button.title = newest ? `Undo ${newest.tool}: ${newest.files.join(", ")}` : "";
}

// Takes back the newest file change; the server refuses when a file changed
// since. The model hears about it with the next message.
async function undoLast() {
  if (state.running) {
    showNotice("A turn is running: wait for it or cancel it before you undo.");
    return;
  }
  try {
    const { changes } = await api(`/sessions/${state.current}/changes`);
    if (!changes.length) {
      showNotice("There is no file change to undo in this session.");
      return;
    }
    const done = await api(`/sessions/${state.current}/undo`, { method: "POST" });
    showInfo(`${done.summary}. The model is told with your next message.`);
  } catch (error) {
    showNotice(error.message);
  }
  reloadFiles().catch(report);
  if (git.loaded) loadGit();
  await loadChanges();
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
      meta.append(el("span", "message-role", "You"));
      body.textContent = entry.input;
      break;
    case "assistant_message":
      node.classList.add("assistant");
      meta.append(el("span", "message-role", "Assistant"));
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

// The navigation uses recorded user messages only, never model-generated HTML.
let messageNavigationFrame = null;
let messageNavigationEntries = [];

function renderMessageNavigation() {
  const host = $("message-anchors");
  host.replaceChildren();
  messageNavigationEntries = [...$("transcript").querySelectorAll(".entry.user")].map((entry, index) => {
    const text = entry.querySelector("pre").textContent.replace(/\s+/gu, " ").trim();
    const summary = text || "Empty message";
    const button = el("button", "message-anchor");
    const number = el("span", "message-anchor-number", String(index + 1).padStart(2, "0"));
    number.setAttribute("aria-hidden", "true");
    button.append(number, el("span", "message-anchor-text", summary));
    button.setAttribute("aria-label", `Question ${index + 1}: ${summary}`);
    button.type = "button";
    button.dataset.tone = index % 6;
    button.title = text;
    button.dataset.seq = entry.dataset.seq;
    button.setAttribute("aria-controls", "transcript");
    button.addEventListener("click", () => {
      const list = $("transcript");
      list.scrollTop += entry.getBoundingClientRect().top - list.getBoundingClientRect().top - 12;
      updateMessageNavigation(entry);
      entry.animate([{ outline: "2px solid var(--accent)" }, { outline: "2px solid transparent" }], 1200);
    });
    host.append(button);
    return { entry, button };
  });
  $("message-nav").hidden = !messageNavigationEntries.length;
  updateMessageNavigation();
}

function updateMessageNavigation(preferred = null) {
  if (!messageNavigationEntries.length) return;
  const list = $("transcript");
  const top = list.getBoundingClientRect().top + 24;
  let current = messageNavigationEntries[0];
  for (const item of messageNavigationEntries) {
    if (item.entry.getBoundingClientRect().top > top) break;
    current = item;
  }
  if (list.scrollHeight > list.clientHeight && list.scrollTop + list.clientHeight >= list.scrollHeight - 2) {
    current = messageNavigationEntries[messageNavigationEntries.length - 1];
  }
  if (preferred) current = messageNavigationEntries.find((item) => item.entry === preferred) || current;
  for (const { button } of messageNavigationEntries) {
    if (button === current.button) button.setAttribute("aria-current", "location");
    else button.removeAttribute("aria-current");
  }
  const index = messageNavigationEntries.indexOf(current);
  $("message-nav-prev").disabled = index === 0;
  $("message-nav-next").disabled = index === messageNavigationEntries.length - 1;
  // Reveal the active summary without scrolling the conversation or page.
  const host = $("message-anchors");
  const bounds = host.getBoundingClientRect();
  const active = current.button.getBoundingClientRect();
  if (active.left < bounds.left) host.scrollLeft += active.left - bounds.left;
  else if (active.right > bounds.right) host.scrollLeft += active.right - bounds.right;
}

async function loadTranscript() {
  if (!state.current) return;
  const session = state.current;
  const records = await api(`/sessions/${session}`);
  if (state.current !== session) return;
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
  renderMessageNavigation();
}

// ---- Turns ----------------------------------------------------------------

function setRunning(running) {
  const was = state.running;
  state.running = running;
  $("send").disabled = running || !state.current;
  $("session-undo").disabled = running;
  $("cancel").hidden = !running;
  if (running && !was) startBusy();
  if (!running) {
    stopBusy();
    $("live").replaceChildren();
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
  document.title = `● ${PAGE_TITLE} — working`;
}

function stopBusy() {
  clearInterval(busy.timer);
  busy.timer = null;
  $("busy").hidden = true;
  document.title = PAGE_TITLE;
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
  const live = $("live");
  // Follow the output unless the user scrolled up to read.
  const following = live.hidden || live.scrollHeight - live.scrollTop - live.clientHeight < 24;
  // Append only the new fragment instead of copying the entire response.
  if (live.lastChild) live.lastChild.appendData(text);
  else live.append(document.createTextNode(text));
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
  { name: "/undo", usage: "/undo", help: "Undo the newest file change of this session" },
  { name: "/help", usage: "/help", help: "Show the documentation and the commands" },
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
        showPanel("plugins");
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
      case "/undo":
        await undoLast();
        break;
      case "/help":
        await openHelp("commands");
        break;
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
      loadChanges().catch(report);
      refreshViews();
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
  const choices = [["Deny", false, false, false], ["Allow", true, false, false]];
  if (request.may_remember) choices.push(["Allow for this session", true, true, false]);
  // "Always allow this command" adds the exact program and arguments to the
  // policy's `commands` (ADR 0066); the server takes them from the call, so
  // the button only follows `command`.
  if (Array.isArray(request.command)) {
    choices.push(["Always allow this command", true, false, true]);
  }
  for (const [label, approve, always, command] of choices) {
    const button = el("button", approve ? "" : "ghost", label);
    button.type = "button";
    if (command) button.title = request.command.join(" ");
    button.addEventListener("click", () => {
      send({ type: "approval", id: request.id, approve, always, command });
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

// ---- Layout -----------------------------------------------------------------

// Where the panels sit (ADR 0062): `GET /api/layout` names the panels of the
// left and the right column, in order; each column shows them as tabs. The
// conversation, the header, the approvals and the status bar are not panels.
// Without an answer (an older server) the page keeps its standard layout.
const STANDARD_LAYOUT = { left: ["sessions"], right: ["files", "plugins"] };
const PANELS = {
  sessions: { label: "Sessions", node: () => $("sessions-panel") },
  files: { label: "Workspace", node: () => $("files-view"), button: "toggle-side" },
  plugins: { label: "Plugins", node: () => $("plugins-panel") },
};
const layout = { left: [], right: [], rightOpen: null };
const REGIONS = {
  left: { box: () => $("left"), tabs: () => $("left-tabs") },
  right: { box: () => $("side"), tabs: () => $("side-tabs") },
};

function panelRegion(id) {
  if (layout.left.includes(id)) return "left";
  if (layout.right.includes(id)) return "right";
  return null;
}

function regionOpen(region) {
  return region !== null && !REGIONS[region].box().hidden;
}

// The panel a column shows, or null.
function regionView(region) {
  return layout[region].find((id) => !PANELS[id].node().hidden) || null;
}

function panelShown(id) {
  const region = panelRegion(id);
  return regionOpen(region) && regionView(region) === id;
}

// Places every panel the page knows; panels the layout does not name wait,
// hidden, outside both columns, so the code that fills them keeps working.
function applyLayout(given) {
  const known = (list) => (Array.isArray(list) ? list : []).filter((id) => PANELS[id]);
  layout.left = [...new Set(known(given.left))];
  layout.right = [...new Set(known(given.right))].filter((id) => !layout.left.includes(id));
  layout.rightOpen = typeof given.right_open === "boolean" ? given.right_open : null;
  for (const [region, { box, tabs }] of Object.entries(REGIONS)) {
    const bar = tabs();
    for (const tab of bar.querySelectorAll("[data-view]")) tab.remove();
    const before = bar.querySelector(".spacer");
    for (const id of layout[region]) {
      const tab = el("button", "", PANELS[id].label);
      tab.type = "button";
      tab.setAttribute("role", "tab");
      tab.dataset.view = id;
      tab.addEventListener("click", () => showPanel(id));
      bar.insertBefore(tab, before);
      PANELS[id].node().hidden = true;
      box().append(PANELS[id].node());
    }
  }
  for (const [id, panel] of Object.entries(PANELS)) {
    if (!panelRegion(id)) {
      panel.node().hidden = true;
      $("unplaced").append(panel.node());
    }
  }
  // The header button opens the right column, whatever it holds; it is named
  // after the workspace when that is there.
  const side = $("toggle-side");
  side.hidden = layout.right.length === 0;
  const sideLabel = layout.right.includes("files") ? "Workspace" : "Side panel";
  side.setAttribute("aria-label", sideLabel);
  side.title = sideLabel;
  // One left panel needs no tabs; an empty left column is not shown.
  $("left-tabs").hidden = layout.left.length < 2;
  $("left").hidden = layout.left.length === 0;
  $("sessions-splitter").hidden = layout.left.length === 0;
  if (layout.right.length === 0) {
    $("side").hidden = true;
    side.setAttribute("aria-expanded", "false");
  }
  selectLeft();
}

// Shows the first left panel, unless one is shown.
function selectLeft() {
  if (regionView("left")) return;
  const first = layout.left[0];
  if (first) selectTab("left", first);
}

function selectTab(region, view) {
  for (const id of layout[region]) PANELS[id].node().hidden = id !== view;
  for (const tab of REGIONS[region].tabs().querySelectorAll("[data-view]")) {
    const current = tab.dataset.view === view;
    tab.classList.toggle("current", current);
    tab.setAttribute("aria-selected", String(current));
  }
}

async function loadLayout() {
  let given = STANDARD_LAYOUT;
  try {
    for (const view of (await api("/ui")).views) addViewPanel(view);
  } catch {
    // Older servers have no views.
  }
  try {
    const reply = await api("/layout");
    given = reply.layout;
    if (reply.problems?.length) showNotice(`Layout: ${reply.problems.join("; ")}`);
  } catch {
    // Older servers have no layout.
  }
  applyLayout(given);
  loadStatusIndicators().catch(() => {});
}

// A plugin change altered the layout (`{type: "layout"}` on the socket):
// applies it in place, keeping the panels shown where they still can be.
async function reloadLayout() {
  const open = !$("side").hidden;
  const right = regionView("right");
  const left = regionView("left");
  await loadLayout();
  if (left && layout.left.includes(left)) showPanel(left);
  if (open) {
    const view = layout.right.includes(right) ? right : preferredSideView();
    if (view) showPanel(view);
  }
}

function sideView() {
  return regionView("right");
}

// The right panel to open: the one shown last, else the first.
function preferredSideView() {
  const view = sidePrefs().view;
  if (layout.right.includes(view)) return view;
  return layout.right[0] || null;
}

function showPanel(view) {
  const region = panelRegion(view);
  if (!region) return;
  if (region === "right") {
    $("side").hidden = false;
    $("toggle-side").setAttribute("aria-expanded", "true");
    saveSidePrefs({ open: true, view });
  }
  selectTab(region, view);
  if (view === "files" && !files.dirs.has("")) loadDir("").catch(treeError);
  if (view === "files" && !git.loaded) loadGit();
  if (view === "plugins") loadPlugins().catch(report);
  if (views.has(view)) renderView(view);
}

// ---- Plugin views -------------------------------------------------------------

// Views other plugins contribute (`GET /api/ui`, ADR 0062), each a panel with
// the id `instance/view`. A view is data from a fixed vocabulary, built here
// with text content only, as the model's replies are; buttons and forms run
// the plugin's actions on the user's click.
const views = new Map();

function addViewPanel(view) {
  if (PANELS[view.id]) return;
  const node = el("div", "view-panel");
  node.setAttribute("role", "tabpanel");
  node.setAttribute("aria-label", view.title);
  node.hidden = true;
  const head = el("div", "panel-head");
  const reload = el("button", "ghost icon", "⟳");
  reload.type = "button";
  reload.title = "Reload the view";
  reload.setAttribute("aria-label", "Reload the view");
  reload.addEventListener("click", () => renderView(view.id));
  head.append(el("h2", "", view.title), reload);
  const message = el("div", "view-message");
  message.hidden = true;
  const body = el("div", "view-body");
  node.append(head, message, body);
  $("unplaced").append(node);
  views.set(view.id, { ...view, node, body, message });
  PANELS[view.id] = { label: view.title, node: () => node };
}

function viewPath(entry) {
  return `/ui/${encodeURIComponent(entry.instance)}/${encodeURIComponent(entry.view)}`;
}

async function renderView(id) {
  const entry = views.get(id);
  if (!entry) return;
  const current = state.current;
  const session = current ? `?session=${encodeURIComponent(current)}` : "";
  try {
    const content = await api(`${viewPath(entry)}${session}`);
    // Another session was opened meanwhile; its own render shows it.
    if (current !== state.current) return;
    entry.body.replaceChildren(...(content.nodes || []).map((node) => viewNode(entry, node)).filter(Boolean));
  } catch (error) {
    if (current !== state.current) return;
    entry.body.replaceChildren(el("p", "note", error.message));
  }
}

// Shown views follow the turns: a turn may change what they show.
function refreshViews() {
  for (const entry of statusIndicators.values()) refreshStatusIndicator(entry);
  for (const id of views.keys()) if (panelShown(id)) renderView(id);
  refreshPluginPage();
}

// A provider said a view changed (`{type: "ui"}` on the socket). A shown
// view renders again once the changes pause; a hidden one renders when shown.
const viewTimers = new Map();
function viewChanged(id) {
  const indicator = statusIndicators.get(id);
  if (indicator) {
    clearTimeout(indicator.timer);
    indicator.timer = setTimeout(() => refreshStatusIndicator(indicator), 150);
  }
  if (pluginPageSelection === id) refreshPluginPage();
  if (!views.has(id) || !panelShown(id)) return;
  clearTimeout(viewTimers.get(id));
  viewTimers.set(id, setTimeout(() => {
    viewTimers.delete(id);
    renderView(id);
  }, 150));
}

async function runAction(entry, action, input, controls) {
  for (const control of controls) control.disabled = true;
  try {
    const outcome = await api(`${viewPath(entry)}/actions/${encodeURIComponent(action)}`, {
      method: "POST",
      body: JSON.stringify({ input, session: state.current || null }),
    });
    if (!entry.workspace || pluginPageSelection === entry.id) {
      entry.message.textContent = outcome.message || "";
      entry.message.hidden = !outcome.message;
      entry.message.classList.remove("bad");
    }
    if (outcome.rerender) {
      if (entry.workspace) {
        if (pluginPageSelection === entry.id) await showPluginPage(entry, outcome.message);
        await loadPlugins();
      } else await renderView(entry.id);
    }
  } catch (error) {
    if (!entry.workspace || pluginPageSelection === entry.id) {
      entry.message.textContent = error.message;
      entry.message.hidden = false;
      entry.message.classList.add("bad");
    }
  } finally {
    for (const control of controls) control.disabled = false;
  }
}

function viewNode(entry, node) {
  const text = (value) => String(value ?? "");
  switch (node.type) {
    case "heading":
      return el("h3", "", text(node.text));
    case "text":
      return el("p", "", text(node.text));
    case "markdown": {
      const box = el("div", "markdown");
      box.append(Markdown.render(text(node.text)));
      return box;
    }
    case "key_value": {
      const list = el("dl", "view-values");
      for (const item of node.items || []) list.append(el("dt", "", text(item.key)), el("dd", "", text(item.value)));
      return list;
    }
    case "table": {
      const table = el("table", "view-table");
      const head = el("tr");
      for (const column of node.columns || []) head.append(el("th", "", text(column)));
      const thead = el("thead");
      thead.append(head);
      const tbody = el("tbody");
      for (const [index, row] of (node.rows || []).entries()) {
        const line = el("tr");
        for (const [column, cell] of row.entries()) {
          const value = el("td", "", text(cell));
          const action = node.row_actions?.[index];
          if (column === 0 && action) {
            const button = el("button", "view-row-select", text(cell));
            button.type = "button";
            button.addEventListener("click", () => runAction(entry, text(action), null, [button]));
            value.replaceChildren(button);
          }
          line.append(value);
        }
        tbody.append(line);
      }
      table.append(thead, tbody);
      return table;
    }
    case "list": {
      const list = el("ul", "view-list");
      for (const item of node.items || []) list.append(el("li", "", text(item)));
      return list;
    }
    case "button": {
      const button = el("button", "ghost", text(node.label));
      button.type = "button";
      button.addEventListener("click", () => runAction(entry, text(node.action), null, [button]));
      return button;
    }
    case "form":
      return viewForm(entry, node);
    default:
      return null;
  }
}

function viewForm(entry, node) {
  const form = el("form", "view-form");
  const inputs = [];
  for (const field of node.fields || []) {
    const label = el("label", "", String(field.label ?? field.name));
    let input;
    if (field.kind === "choice") {
      input = el("select");
      for (const option of field.options || []) {
        const choice = el("option", "", String(option));
        choice.value = String(option);
        input.append(choice);
      }
      input.value = String(field.value ?? "");
    } else {
      input = el("input");
      input.type = field.kind === "boolean" ? "checkbox" : field.kind === "number" ? "number" : "text";
      if (field.kind === "boolean") input.checked = field.value === "true";
      else input.value = String(field.value ?? "");
    }
    input.name = String(field.name);
    label.append(input);
    form.append(label);
    inputs.push([field, input]);
  }
  const submit = el("button", "", String(node.submit || "Submit"));
  submit.type = "submit";
  form.append(submit);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const values = {};
    for (const [field, input] of inputs) {
      if (field.kind === "boolean") values[field.name] = input.checked;
      else if (field.kind === "number") values[field.name] = input.value === "" ? null : Number(input.value);
      else values[field.name] = input.value;
    }
    runAction(entry, String(node.action), values, [submit]);
  });
  return form;
}

function hideSide() {
  $("side").hidden = true;
  $("toggle-side").setAttribute("aria-expanded", "false");
  saveSidePrefs({ open: false });
}

// ---- Internal APIs --------------------------------------------------------

// The internal APIs (API groups) as one choice above the message box: "Off"
// or one API. Only one can be connected, and the line beside it says what
// the model can reach right now.
function renderApiGroups() {
  const box = $("api-groups");
  const offered = state.groups;
  $("api-bar").hidden = offered.length === 0;
  const connected = offered.find((group) => group.active) || null;
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
  box.replaceChildren(
    choice("Off", connected === null, "Connect no internal API", () => switchGroup(connected.id, false)),
    ...offered.map((group) =>
      choice(group.label, group.active, `Connect the ${group.label} API (disconnects any other)`, () =>
        switchGroup(group.id, true),
      ),
    ),
  );
  const line = $("api-state");
  line.textContent = connected
    ? `Connected to ${connected.label}: the model can query this API.`
    : "No API connected: the model cannot reach any internal API.";
  line.classList.toggle("connected", connected !== null);
}

// A group switch starts and stops plugins: show it in the plugin list.
function refreshPluginList() {
  if (panelShown("plugins")) loadPlugins().catch(report);
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

// The header buttons open their view, or close the panel when it shows it.
// The header button: closes the right column, or opens it on the workspace
// (else on the panel shown last).
function toggleRight() {
  if (!$("side").hidden) hideSide();
  else {
    const view = layout.right.includes("files") ? "files" : preferredSideView();
    if (view) showPanel(view);
  }
}

// Every plugin: the Plugins panel where the layout has it, else the same
// workspace in Settings.
function showPlugins() {
  if (panelRegion("plugins")) showPanel("plugins");
  else openSettings("plugins");
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
  if (panelShown("files")) {
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
  if (!panelShown("files")) showPanel("files");
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

// The wrap switch (the file and the activity log) and the theme come from
// the preferences, so the browser and the desktop agree.
function applyWrap() {
  const wrap = Boolean(state.prefs.wrap_files);
  $("wrap-files").setAttribute("aria-pressed", String(wrap));
  $("activity").classList.toggle("wrap", wrap);
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

function toggleWrap() {
  const wrap = !state.prefs.wrap_files;
  state.prefs.wrap_files = wrap;
  state.prefs.wrap_activity = wrap;
  savePrefs();
  applyWrap();
  renderFile();
}

// The activity log is folded to its heading until opened; the choice is
// kept with the side panel's layout.
function applyActivityFold() {
  const open = Boolean(sidePrefs().activity_open);
  $("activity").hidden = !open;
  $("view-splitter").hidden = !open;
  $("activity-toggle").setAttribute("aria-expanded", String(open));
}

function toggleActivity() {
  saveSidePrefs({ activity_open: !sidePrefs().activity_open });
  applyActivityFold();
  if (sidePrefs().activity_open) $("activity").scrollTop = $("activity").scrollHeight;
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
    if (known && git.loaded && regionOpen(panelRegion("files"))) loadGit();
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
    statusIndicatorsConnected = true;
    loadStatusIndicators().catch(() => {});
    state.retry = 500;
    // Shown only while the link to the local RAH process is down.
    $("connection").textContent = "connected";
    $("connection").hidden = true;
    // Keep other notices, such as a failed startup request.
    if ($("notice-text").textContent === NOT_CONNECTED) showNotice("");
    // A switch made while this window was not connected sent no notice here.
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
    } else if (data.type === "layout") {
      reloadLayout().catch(report);
    } else if (data.type === "ui") {
      viewChanged(`${data.instance}/${data.view}`);
    } else if (data.type === "host_notice") {
      // A plugin skipped something quietly that the user can act on.
      showTransientNotice(`${data.plugin}: ${data.message}`, 20000);
    }
    else if (data.type === "error") {
      showNotice(data.message);
      setRunning(false);
    }
  });
  socket.addEventListener("close", () => {
    statusIndicatorsConnected = false;
    for (const entry of statusIndicators.values()) {
      ++entry.request;
      paintStatusIndicator(entry, { state: "warning", label: entry.title, detail: `${entry.title}: connection lost` });
    }
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

// The catalog's examples supply control types, not values to apply. The JSON
// editor is the canonical draft; every field change updates that same draft.
let pluginFieldControls = [];
let pluginSelection = null;
let pluginPageSelection = null;
let pluginPages = [];
let pluginPageRequest = 0;

function refreshPluginPage() {
  const page = pluginPages.find((entry) => entry.id === pluginPageSelection);
  if (page && ($("settings-dialog").open && settings.tab === "plugins" || panelShown("plugins"))) {
    showPluginPage(page);
  }
}

function configObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function pluginMessage(message, bad = false) {
  const node = $("plugin-message");
  node.textContent = message;
  node.hidden = !message;
  node.classList.toggle("bad", bad);
}

function buildPluginFields() {
  const value = JSON.parse($("editor-config").value || "null");
  const fields = $("editor-fields");
  fields.replaceChildren();
  pluginFieldControls = [];
  if (value !== null && !configObject(value)) return false;
  const current = value || {};
  const example = configExamples[editorKind()];
  const samples = configObject(example) ? example : {};
  const help = configHelp[editorKind()] || [];
  const keys = new Set([...help.map((entry) => entry.key), ...Object.keys(samples), ...Object.keys(current)]);
  for (const key of keys) {
    const present = Object.hasOwn(current, key);
    const sample = present ? current[key] : samples[key];
    const type = typeof sample === "boolean" ? "boolean" : typeof sample === "number" ? "number" : typeof sample === "string" ? "string" : "json";
    const row = el("div", "plugin-field");
    const override = el("input");
    override.type = "checkbox";
    override.checked = present;
    override.setAttribute("aria-label", `Set ${key}`);
    const heading = el("label", "plugin-field-heading");
    heading.append(override, el("span", "", key));
    const input = el(type === "json" || (type === "string" && String(sample).includes("\n")) ? "textarea" : "input");
    if (input.tagName === "INPUT") input.type = type === "boolean" ? "checkbox" : type === "number" ? "number" : "text";
    if (type === "number") input.step = "any";
    input.dataset.configKey = key;
    input.setAttribute("aria-label", key);
    input.disabled = !present;
    input.spellcheck = false;
    if (type === "boolean") input.checked = present && sample;
    else input.value = present ? (type === "json" ? JSON.stringify(sample, null, 2) : String(sample)) : "";
    if (type !== "boolean") input.placeholder = Object.hasOwn(samples, key)
      ? `Example: ${type === "json" ? JSON.stringify(samples[key]) : samples[key]}`
      : type === "json" ? "JSON value" : "Value";
    const hint = help.find((entry) => entry.key === key)?.help;
    row.append(heading, input);
    if (hint) row.append(el("small", "hint", hint));
    fields.append(row);
    const control = { key, type, override, input };
    pluginFieldControls.push(control);
    const update = () => {
      input.disabled = !override.checked;
      for (const field of pluginFieldControls) field.input.setCustomValidity("");
      let invalid = input;
      try {
        const entries = [];
        for (const field of pluginFieldControls) {
          if (!field.override.checked) continue;
          invalid = field.input;
          let next;
          if (field.type === "boolean") next = field.input.checked;
          else if (field.type === "number") {
            next = Number(field.input.value);
            if (!field.input.value.trim() || !Number.isFinite(next)) throw new Error(`${field.key}: enter a number`);
          } else if (field.type === "json") {
            try { next = JSON.parse(field.input.value); }
            catch { throw new Error(`${field.key}: enter valid JSON`); }
          } else next = field.input.value;
          entries.push([field.key, next]);
        }
        const next = Object.fromEntries(entries);
        $("editor-config").value = entries.length || value !== null ? JSON.stringify(next, null, 2) : "";
        pluginMessage("");
      } catch (error) {
        invalid.setCustomValidity(error.message);
        pluginMessage(error.message, true);
      }
    };
    override.addEventListener("change", update);
    input.addEventListener("input", update);
  }
  if (!keys.size) fields.append(el("p", "hint", "No described settings. Use Advanced JSON to add configuration keys."));
  fields.prepend(el("p", "hint", "Check a setting to set its value. Unchecked settings use the plugin's defaults; examples are not applied."));
  return true;
}

function setPluginEditorFormat(format) {
  if (format === "json" && !$("editor-fields").hidden) {
    const invalid = pluginFieldControls.find((field) => !field.input.disabled && !field.input.validity.valid);
    if (invalid) {
      $("editor-format").value = "fields";
      invalid.input.reportValidity();
      return;
    }
  }
  try {
    if (format === "fields" && !buildPluginFields()) format = "json";
  } catch (error) {
    pluginMessage(`The configuration is not valid JSON: ${error.message}`, true);
    format = "json";
  }
  $("editor-format").value = format;
  $("editor-fields").hidden = format !== "fields";
  $("editor-json-row").hidden = format !== "json";
  // Hidden fields must not block an advanced JSON save.
  for (const field of pluginFieldControls) {
    field.input.disabled = format !== "fields" || !field.override.checked;
  }
}

function filterPluginList() {
  const query = $("plugin-search").value.trim().toLowerCase();
  for (const row of $("plugins").querySelectorAll("tr:not(.group)")) {
    row.hidden = !row.dataset.search.includes(query);
  }
  for (const group of $("plugins").querySelectorAll("tr.group")) {
    let row = group.nextElementSibling;
    let visible = false;
    while (row && !row.classList.contains("group")) {
      visible ||= !row.hidden;
      row = row.nextElementSibling;
    }
    group.hidden = !visible;
  }
  for (const button of $("plugin-pages").querySelectorAll("button")) {
    button.hidden = !button.dataset.search.includes(query);
  }
  const visiblePages = [...$("plugin-pages").querySelectorAll("button")].some((button) => !button.hidden);
  const heading = $("plugin-pages").querySelector("h3");
  if (heading) heading.hidden = !visiblePages;
  $("plugin-empty").hidden = visiblePages || [...$("plugins").querySelectorAll("tr:not(.group)")].some((row) => !row.hidden);
}

async function showPluginPage(page, message = "") {
  const request = ++pluginPageRequest;
  const changed = pluginPageSelection !== page.id;
  pluginPageSelection = page.id;
  pluginSelection = null;
  for (const row of $("plugins").querySelectorAll("tr")) row.classList.remove("selected");
  for (const button of $("plugin-pages").querySelectorAll("button")) {
    button.classList.toggle("current", button.dataset.page === page.id);
    if (changed && button.dataset.page === page.id) button.scrollIntoView({ block: "nearest" });
  }
  if (changed) $("plugin-page").parentElement.scrollTop = 0;
  $("editor").hidden = true;
  $("plugin-summary").replaceChildren(el("h3", "", page.title));
  const body = $("plugin-page");
  body.hidden = false;
  body.replaceChildren(el("p", "hint", "Loading…"));
  pluginMessage(message);
  const entry = { ...page, body, message: $("plugin-message"), workspace: true };
  try {
    const session = state.current ? `?session=${encodeURIComponent(state.current)}` : "";
    const content = await api(`${viewPath(entry)}${session}`);
    if (request !== pluginPageRequest) return;
    body.replaceChildren(...(content.nodes || []).map((node) => viewNode(entry, node)).filter(Boolean));
  } catch (error) {
    if (request === pluginPageRequest) pluginMessage(error.message, true);
  }
}

function selectPlugin(plugin) {
  ++pluginPageRequest;
  pluginPageSelection = null;
  pluginSelection = plugin.id;
  for (const button of $("plugin-pages").querySelectorAll("button")) button.classList.remove("current");
  $("plugin-page").hidden = true;
  pluginMessage("");
  renderPluginSummary(plugin);
  if (plugin.kind && !plugin.locked && plugin.state !== "stopped") openEditor("configure", plugin);
  else $("editor").hidden = true;
  for (const row of $("plugins").querySelectorAll("tr")) row.classList.toggle("selected", row.dataset.plugin === plugin.id);
}

function renderPluginSummary(plugin) {
  const summary = $("plugin-summary");
  summary.replaceChildren(el("h3", "", plugin.id), el("p", "hint", `${plugin.kind || "Unknown kind"} · ${plugin.state}${plugin.detail ? `: ${plugin.detail}` : ""}`));
  summary.append(pluginActions(plugin));
  if (plugin.locked) summary.append(el("p", "hint", "This plugin is managed by the host and cannot be changed here."));
  if (plugin.state === "stopped") summary.append(el("p", "hint", "Start this plugin to edit its configuration."));
}

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
  if (editorKind() === "ui-layout-config") showLayoutPanels();
}

// For a layout plugin: every panel id it can place (`panels` of
// `GET /api/layout`), with where each is now.
let panelsShown = 0;
async function showLayoutPanels() {
  const asked = ++panelsShown;
  let panels;
  try {
    panels = (await api("/layout")).panels || [];
  } catch {
    return;
  }
  // A later call, or another kind, replaced the help meanwhile.
  if (asked !== panelsShown || editorKind() !== "ui-layout-config") return;
  const list = el("dd", "panel-ids");
  for (const panel of panels) {
    const where = panel.column ? `now ${panel.column}` : "not shown";
    const from = panel.kind === "view" ? `view of ${panel.plugin}` : "built-in";
    const line = el("div");
    line.append(el("code", "", panel.id), ` ${panel.title} (${from}; ${where})`);
    list.append(line);
  }
  $("editor-help").hidden = false;
  $("editor-help-list").append(el("dt", "", "Panels you can place"), list);
}

// Copies the example into the editor to start from.
function fillConfigExample() {
  const example = configExamples[editorKind()];
  const textarea = $("editor-config");
  if (example === undefined) return;
  if (textarea.value.trim() && !window.confirm("Replace the current configuration with the example?")) return;
  textarea.value = JSON.stringify(example, null, 2);
  showConfigExample();
  setPluginEditorFormat($("editor-format").value);
  const missing = placeholderValues(example);
  pluginMessage(missing.length ? `Replace ${missing.join(", ")} before saving.` : "", missing.length > 0);
  textarea.focus();
}

function report(error) {
  showNotice(error.message);
  pluginMessage(error.message, true);
}

// Help: the documentation built into the server (`/api/docs`) and, last in
// the list, the slash commands. Documents are loaded when first shown.
const HELP_COMMANDS = "commands";
const help = { docs: null, current: null, loaded: new Map() };

// The anchor a heading gets on the usual Markdown hosts.
function headingAnchor(text) {
  return text.trim().toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").replace(/\s/g, "-");
}

function helpScroll(anchor) {
  const heading = [...$("help-body").querySelectorAll("h1, h2, h3, h4, h5, h6")]
    .find((candidate) => headingAnchor(candidate.textContent) === anchor);
  if (heading) heading.scrollIntoView({ block: "start" });
}

function decoded(text) {
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}

// What a relative link of the shown document does: `#anchor` scrolls, and a
// path that names another built-in document opens it. Other links stay text.
function helpLink(url) {
  if (url.startsWith("#")) return () => helpScroll(decoded(url.slice(1)));
  const from = (help.docs || []).find((doc) => doc.id === help.current);
  if (!from) return null;
  let target;
  try {
    target = new URL(url, `http://docs.invalid/${from.path}`);
  } catch {
    return null;
  }
  if (target.host !== "docs.invalid") return null;
  const path = decoded(target.pathname.slice(1));
  const doc = help.docs.find((candidate) => candidate.path === path);
  return doc ? () => showHelp(doc.id, decoded(target.hash.slice(1))) : null;
}

function helpCommands() {
  const list = el("dl");
  for (const command of SLASH_COMMANDS) {
    const usage = el("dt");
    usage.append(el("code", "", command.usage));
    list.append(usage, el("dd", "", command.help));
  }
  const page = document.createDocumentFragment();
  page.append(
    el("h1", "", "Commands"),
    el("p", "", "Type these in the message box. Typing / lists them; Tab or Enter completes one."),
    list,
  );
  return page;
}

async function showHelp(id, anchor) {
  help.current = id;
  for (const button of $("help-nav").children) {
    const current = button.dataset.doc === id;
    button.classList.toggle("current", current);
    if (current) button.setAttribute("aria-current", "page");
    else button.removeAttribute("aria-current");
  }
  const body = $("help-body");
  if (id === HELP_COMMANDS) {
    body.replaceChildren(helpCommands());
  } else {
    let doc = help.loaded.get(id);
    if (!doc) {
      try {
        doc = await api(`/docs/${encodeURIComponent(id)}`);
      } catch (error) {
        if (help.current === id) body.replaceChildren(el("p", "", error.message));
        return;
      }
      help.loaded.set(id, doc);
    }
    // Another document was chosen meanwhile.
    if (help.current !== id) return;
    body.replaceChildren(Markdown.render(doc.markdown, { breaks: false, link: helpLink }));
  }
  if (anchor) helpScroll(anchor);
  else body.scrollTop = 0;
}

// Opens Help on `id`, else on the document shown last, else on the first.
async function openHelp(id) {
  $("help-dialog").showModal();
  if (!help.docs) {
    let docs;
    try {
      docs = await api("/docs");
    } catch (error) {
      $("help-body").replaceChildren(el("p", "", error.message));
      return;
    }
    help.docs = docs;
    const entries = [...docs, { id: HELP_COMMANDS, title: "Commands" }];
    $("help-nav").replaceChildren(...entries.map((doc) => {
      const button = el("button", "", doc.title);
      button.type = "button";
      button.dataset.doc = doc.id;
      button.addEventListener("click", () => showHelp(doc.id));
      return button;
    }));
  }
  await showHelp(id || help.current || help.docs[0]?.id || HELP_COMMANDS);
}

function setupHelp() {
  $("open-help").addEventListener("click", () => openHelp());
  $("help-close").addEventListener("click", () => $("help-dialog").close());
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
  ++pluginPageRequest;
  pluginPageSelection = null;
  $("plugin-page").hidden = true;
  pluginMessage("");
  if (mode === "add") {
    pluginSelection = null;
    $("plugin-summary").replaceChildren();
  }
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
  setPluginEditorFormat("fields");
  showIdHint();
  $("editor").hidden = false;
  $("editor").parentElement.scrollTop = 0;
  (mode === "configure" ? $("editor-format") : $("editor-id")).focus();
}

async function saveEditor(event) {
  event.preventDefault();
  let config;
  try {
    config = JSON.parse($("editor-config").value || "null");
  } catch (error) {
    pluginMessage(`The configuration is not valid JSON: ${error.message}`, true);
    return;
  }
  const missing = placeholderValues(config);
  if (missing.length) {
    pluginMessage(`Replace ${missing.join(", ")} before saving.`, true);
    return;
  }
  const kind = $("editor-kind").value;
  const id = $("editor-id").value.trim() || null;
  if (editor.mode === "add" && kind === "tools-http" && !id) {
    pluginMessage("Name this API group (for example http-ddt) so its button can be told apart.", true);
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
    pluginMessage(`${editor.target?.id || id || kind}: configuration saved.`);
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

// A process plugin the user wrote, or had written, for themselves
// (`personal = true`, as `plugin-builder` writes it in `mount.toml`).
function isPersonal(plugin) {
  return plugin.kind === "process" && plugin.config?.personal === true;
}

function pluginGroup(label, className) {
  const row = el("tr", `group ${className}`);
  const cell = el("th", "", label);
  cell.colSpan = 2;
  row.append(cell);
  return row;
}

function pluginRow(plugin) {
  const personal = isPersonal(plugin);
  const row = el("tr", personal ? "personal" : "");
  row.dataset.plugin = plugin.id;
  row.dataset.search = `${plugin.id} ${plugin.kind || ""} ${plugin.state} ${personal ? "personal" : ""}`.toLowerCase();
  row.classList.toggle("selected", pluginSelection === plugin.id);
  const name = el("td");
  const choose = el("button", "plugin-select", plugin.id);
  choose.type = "button";
  choose.addEventListener("click", () => selectPlugin(plugin));
  name.append(choose);
  if (personal) {
    const tag = el("span", "personal-tag", "Personal");
    tag.title = "A plugin you wrote, or had written, for yourself";
    name.append(tag);
  }
  if (plugin.kind && plugin.kind !== plugin.id) name.append(el("span", "kind", plugin.kind));
  const cell = el("td", `state-${plugin.state}`, plugin.state);
  if (plugin.detail) cell.title = plugin.detail;
  row.append(name, cell);
  return row;
}

function pluginActions(plugin) {
  const actions = el("div", "row-actions");
  if (plugin.kind && !plugin.locked) {
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
  }
  return actions;
}

async function loadCatalog() {
  try {
    const selected = $("editor-kind").value;
    const catalog = await api("/catalog");
    configExamples = catalog.examples || {};
    configHelp = catalog.help || {};
    $("editor-kind").replaceChildren(...catalog.kinds.map((kind) => {
      const option = el("option", "", kind);
      option.value = kind;
      return option;
    }));
    if (catalog.kinds.includes(selected)) $("editor-kind").value = selected;
    $("add-plugin").hidden = false;
  } catch {
    $("add-plugin").hidden = true;
  }
}

async function loadPlugins() {
  const [plugins, info, ui] = await Promise.all([api("/plugins"), api("/info"), api("/ui")]);
  pluginPages = ui.views || [];
  loadStatusIndicators().catch(() => {});
  // Personal plugins are listed apart, after the distribution's own.
  const personal = plugins.filter(isPersonal);
  const rows = plugins.filter((plugin) => !isPersonal(plugin)).map(pluginRow);
  if (personal.length > 0) {
    rows.unshift(pluginGroup("RAH plugins", "builtin"));
    rows.push(pluginGroup("Personal plugins", "personal"), ...personal.map(pluginRow));
  }
  $("plugins").replaceChildren(...rows);
  $("plugin-pages").replaceChildren(...pluginPages.map((page) => {
    const button = el("button", "plugin-select", page.title);
    button.type = "button";
    button.title = `${page.instance}/${page.view}`;
    button.dataset.page = page.id;
    button.classList.toggle("current", page.id === pluginPageSelection);
    button.dataset.search = `${page.title} ${page.instance} ${page.view}`.toLowerCase();
    button.addEventListener("click", () => showPluginPage(page));
    return button;
  }));
  if (pluginPages.length) $("plugin-pages").prepend(el("h3", "", "Plugin pages"));
  filterPluginList();
  const selected = plugins.find((plugin) => plugin.id === pluginSelection);
  if (selected) {
    renderPluginSummary(selected);
    if (selected.state === "stopped" || selected.locked) $("editor").hidden = true;
  }
  else if (pluginSelection || pluginPageSelection && !pluginPages.some((page) => page.id === pluginPageSelection)) {
    pluginSelection = null;
    pluginPageSelection = null;
    ++pluginPageRequest;
    $("editor").hidden = true;
    $("plugin-page").hidden = true;
    $("plugin-summary").replaceChildren(el("p", "hint", "Select a plugin to view its settings, or open a plugin page."));
  } else if (!pluginPageSelection && $("editor").hidden) {
    $("plugin-summary").replaceChildren(el("p", "hint", "Select a plugin to view its settings, or open a plugin page."));
  }
  $("tools").replaceChildren(...info.tools.map((tool) => el("li", "", `${tool.name} (${tool.permission})`)));
}

// ---- Status line ----------------------------------------------------------

// Providers opt in through UiViews; this footer knows no integration ids/files.
const statusIndicators = new Map();
let statusIndicatorsRequest = 0;
let statusIndicatorsConnected = true;

function indicatorIcon(kind) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("aria-hidden", "true");
  const line = document.createElementNS(ns, "path");
  line.setAttribute("d", kind === "graph" ? "M4 4L12 4L8 12Z" : "M1 8h3l2-5 4 10 2-5h3");
  line.setAttribute("fill", "none");
  line.setAttribute("stroke", "currentColor");
  line.setAttribute("stroke-width", "1.3");
  svg.append(line);
  if (kind === "graph") {
    for (const [x, y] of [[4, 4], [12, 4], [8, 12]]) {
      const circle = document.createElementNS(ns, "circle");
      circle.setAttribute("cx", String(x)); circle.setAttribute("cy", String(y));
      circle.setAttribute("r", "2"); circle.setAttribute("fill", "currentColor");
      svg.append(circle);
    }
  }
  return svg;
}

function paintStatusIndicator(entry, value) {
  const states = ["idle", "busy", "ready", "warning", "error"];
  const state = states.includes(value.state) ? value.state : "warning";
  entry.icon = value.icon || entry.icon || "activity";
  const label = String(value.label || entry.title).slice(0, 64);
  const detail = String(value.detail || entry.title).slice(0, 1024);
  entry.node.dataset.state = state;
  entry.node.title = `${detail}\nClick to open ${entry.title}`;
  entry.node.setAttribute("aria-label", detail.split("\n")[0] || label);
  entry.node.replaceChildren(indicatorIcon(entry.icon), el("span", "indicator-label", label));
}

async function refreshStatusIndicator(entry) {
  if (!statusIndicatorsConnected) return;
  const request = ++entry.request;
  const current = state.current;
  const workspace = state.workspace;
  try {
    const session = current ? `?session=${encodeURIComponent(current)}` : "";
    const content = await api(`${viewPath(entry)}${session}`);
    if (statusIndicators.get(entry.id) !== entry || request !== entry.request || !statusIndicatorsConnected
      || current !== state.current || (workspace !== undefined && workspace !== state.workspace)) return;
    const item = (content.nodes || []).find((node) => node.type === "status");
    paintStatusIndicator(entry, item || { state: "warning", label: entry.title, detail: `${entry.title}: no status reported` });
  } catch (error) {
    if (statusIndicators.get(entry.id) === entry && request === entry.request) {
      paintStatusIndicator(entry, { state: "warning", label: entry.title, detail: `${entry.title}: status unavailable\n${error.message}` });
    }
  }
}

async function loadStatusIndicators() {
  const request = ++statusIndicatorsRequest;
  let specs;
  try { specs = (await api("/ui")).views.filter((view) => view.status_bar).slice(0, 4); }
  catch (error) {
    if (request !== statusIndicatorsRequest) return;
    for (const entry of statusIndicators.values()) {
      ++entry.request;
      paintStatusIndicator(entry, { state: "warning", label: entry.title, detail: `${entry.title}: status unavailable\n${error.message}` });
    }
    return;
  }
  if (request !== statusIndicatorsRequest) return;
  for (const [id, entry] of statusIndicators) {
    if (!specs.some((view) => view.id === id)) {
      clearTimeout(entry.timer); entry.node.remove(); statusIndicators.delete(id);
    }
  }
  const container = $("status-plugins");
  for (const spec of specs) {
    let entry = statusIndicators.get(spec.id);
    if (!entry) {
      const button = el("button", "item link status-indicator");
      button.type = "button"; button.dataset.indicator = spec.id;
      entry = { ...spec, node: button, request: 0 };
      button.addEventListener("click", () => { showPlugins(); showPluginPage(entry); });
      statusIndicators.set(spec.id, entry);
      paintStatusIndicator(entry, { state: "idle", label: spec.title, detail: `${spec.title}: loading status` });
    }
    container.append(entry.node);
    refreshStatusIndicator(entry);
  }
  container.hidden = statusIndicators.size === 0;
}

function statusItem(id, text, title, className) {
  const node = $(id);
  node.textContent = text;
  node.title = title || text;
  node.className = `item${className ? ` ${className}` : ""}${id === "status-root" ? " path link" : id === "status-git" || id === "status-agent" ? " link" : ""}`;
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
  // While plugins restart, the status may briefly not know a provider's kind
  // (nor its model); such an answer is no switch.
  if (provider && !provider.kind) return;
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
  announceProvider("model", status.model);
  announceProvider("loop", status.loop);
  const [text, title, bad] = agentText(status);
  statusItem("status-agent", text, title, bad ? "bad" : "");
  for (const entry of statusIndicators.values()) refreshStatusIndicator(entry);
}

// The loop, the tools and the policy in one status item: the tool count,
// and the loop or the policy only when they are not the usual ones. The
// title tells all three.
function agentText(status) {
  const [loop, loopTitle] = providerText("loop", status.loop);
  const parts = [`${status.tools} tools`];
  if (!status.loop || status.loop.kind !== "loop-react") parts.push(loop);
  // Without a policy, calls run unchecked, or are all denied when the
  // configuration expects one (it failed or is between two starts).
  const stopped = !status.guarded && status.guard_required;
  if (!status.guarded) parts.push(stopped ? "NO POLICY: tools denied" : "UNGUARDED");
  const title = [
    loopTitle,
    `${status.tools} tools offered to the model`,
    status.guarded
      ? "Guarded: a policy plugin decides every tool call"
      : stopped
        ? "The policy plugin is not running: every tool call is denied until it runs again"
        : "UNGUARDED: no policy plugin, tool calls run without checks",
    "Click to see every plugin",
  ].join("\n");
  return [parts.join(" · "), title, !status.loop || !status.guarded];
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
  setupHelp();
  try {
    state.prefs = { ...state.prefs, ...(await api("/preferences")) };
  } catch {
    // Older servers have no preferences; the defaults apply.
  }
  applyTheme();
  $("session-search").addEventListener("input", searchSessions);
  for (const [id, direction] of [["message-nav-prev", -1], ["message-nav-next", 1]]) {
    $(id).addEventListener("click", () => {
      const index = messageNavigationEntries.findIndex(({ button }) => button.hasAttribute("aria-current"));
      messageNavigationEntries[index + direction]?.button.click();
    });
  }
  $("transcript").addEventListener("scroll", () => {
    if (messageNavigationFrame !== null) return;
    messageNavigationFrame = requestAnimationFrame(() => {
      messageNavigationFrame = null;
      updateMessageNavigation();
    });
  }, { passive: true });
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
  $("session-undo").addEventListener("click", () => undoLast().catch(report));
  $("session-delete").addEventListener("click", () => {
    const info = currentInfo();
    if (info) deleteSession(info).catch(report);
  });
  for (const format of ["md", "json"]) {
    $(`export-${format}`).addEventListener("click", () => exportSession(format).catch(report));
  }
  // The session menu closes once an item is chosen, on Escape, and on a
  // click elsewhere.
  const sessionMenu = $("session-menu");
  sessionMenu.querySelector(".menu-items").addEventListener("click", () => { sessionMenu.open = false; });
  sessionMenu.addEventListener("keydown", (event) => {
    if (event.key === "Escape") sessionMenu.open = false;
  });
  document.addEventListener("click", (event) => {
    if (sessionMenu.open && !sessionMenu.contains(event.target)) sessionMenu.open = false;
  });
  $("toggle-side").addEventListener("click", toggleRight);
  $("status-agent").addEventListener("click", showPlugins);
  $("side-close").addEventListener("click", hideSide);
  $("notice-close").addEventListener("click", () => showNotice(""));
  $("tree-refresh").addEventListener("click", () => reloadFiles().catch(report));
  $("close-all").addEventListener("click", closeAllFiles);
  $("wrap-files").addEventListener("click", toggleWrap);
  applyWrap();
  $("activity-toggle").addEventListener("click", toggleActivity);
  applyActivityFold();
  $("activity-clear").addEventListener("click", () => $("activity").replaceChildren());
  setupLayout();
  $("git-refresh").addEventListener("click", () => loadGit());
  renderFile();
  renderGit();
  $("refresh-plugins").addEventListener("click", () => loadPlugins().catch(report));
  $("expand-plugins").addEventListener("click", () => openSettings("plugins"));
  $("plugin-search").addEventListener("input", filterPluginList);
  $("editor-format").addEventListener("change", () => setPluginEditorFormat($("editor-format").value));
  $("add-plugin").addEventListener("click", () => openEditor("add", null));
  $("editor").addEventListener("submit", saveEditor);
  $("editor-kind").addEventListener("change", () => {
    showConfigExample();
    showIdHint();
    setPluginEditorFormat($("editor-format").value);
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
  await loadLayout();
  await loadGroups();
  const view = preferredSideView();
  if (view && (sidePrefs().open ?? layout.rightOpen ?? window.innerWidth >= 1100)) showPanel(view);
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
