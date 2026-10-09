"use strict";

// Presentation only. Never persist repository, runtime or authority state here.
const DesktopLayout = (() => {
  const key = "rah.desktop.layout.v1";
  const tabs = ["runtime", "authority", "activity", "repository", "settings"];
  const defaults = () => ({ version: 1, left: 240, right: 340, leftCollapsed: false, rightCollapsed: false, tab: "runtime" });
  function validate(value) {
    if (!value || value.version !== 1 || !Number.isFinite(value.left) || !Number.isFinite(value.right)
      || value.left < 180 || value.left > 480 || value.right < 260 || value.right > 640
      || typeof value.leftCollapsed !== "boolean" || typeof value.rightCollapsed !== "boolean"
      || !tabs.includes(value.tab)) return defaults();
    return { version: 1, left: value.left, right: value.right, leftCollapsed: value.leftCollapsed, rightCollapsed: value.rightCollapsed, tab: value.tab };
  }
  function bounds(side, value, available, other) {
    const minimum = side === "left" ? 180 : 260;
    return Math.max(minimum, Math.min(value, side === "left" ? 480 : 640, available - other - 320 - 12));
  }
  function install() {
    const $ = (s) => document.querySelector(s);
    let state;
    try { state = validate(JSON.parse(localStorage.getItem(key))); } catch { state = defaults(); }
    const shell = $(".status-card");
    // Retain this before its Runtime ancestor moves into the detached Inspector.
    const connectionError = $("#connection-error");
    const toolbar = document.createElement("header");
    toolbar.className = "workspace-toolbar";
    toolbar.innerHTML = '<strong id="application-title">RAH</strong><span id="layout-repository"></span><span id="layout-runtime"></span><button id="toggle-workspace" type="button">Workspace</button><button id="toggle-inspector" type="button">Inspector</button><button id="focus-chat" type="button">Focus Chat</button><button id="reset-layout" type="button">Reset Layout</button>';
    document.body.prepend(toolbar);
    toolbar.append($("#codex-connection"));
    const left = document.createElement("aside"); left.id = "workspace-panel"; left.setAttribute("aria-label", "Workspace");
    const center = $(".chat"); center.id = "chat-panel";
    const right = document.createElement("aside"); right.id = "inspector-panel"; right.setAttribute("aria-label", "Inspector");
    left.innerHTML = '<h2>Workspace</h2>';
    const repo = $(".repository");
    // Keep delegated repository handlers on their original ancestor.
    const navigation = document.createElement("div"); navigation.className = "workspace-navigation";
    const membership = $("#repository-membership-title").closest("section");
    navigation.append(membership, $("#choose-repository"), $("#repository-path"), $("#repository-error"));
    left.append(navigation, $(".remembered-workspaces"));
    const history = document.createElement("details"); history.className = "conversation-history";
    history.innerHTML = '<summary>Conversation history and context</summary>';
    for (const id of ["resume-previous-conversation", "resume-conversation-hint", "clear-conversation-history"]) history.append($("#" + id));
    left.append(history);
    const newConversation = $("#new-conversation"); center.prepend(newConversation);
    const tablist = document.createElement("div"); tablist.className = "inspector-tabs"; tablist.setAttribute("role", "tablist"); tablist.setAttribute("aria-label", "Inspector"); right.append(tablist);
    const sections = {
      runtime: [$("#runtime-status-title").closest("section"), $("#legacy-model-configuration")],
      authority: [$(".authority-review")], activity: [$(".activity")], repository: [repo],
      settings: [$(".profile"), $(".commit-identity"), $("#application-status-title").closest("section")],
    };
    for (const tab of tabs) {
      const button = document.createElement("button"); button.type = "button"; button.id = "tab-" + tab;
      button.textContent = { runtime: "Runtime / Model", authority: "Authority / Tools", activity: "Activity", repository: "Status / Review", settings: "Settings" }[tab];
      button.setAttribute("role", "tab"); button.setAttribute("aria-controls", "panel-" + tab); tablist.append(button);
      const panel = document.createElement("div"); panel.id = "panel-" + tab; panel.setAttribute("role", "tabpanel"); panel.setAttribute("aria-labelledby", button.id); panel.tabIndex = 0;
      panel.append(...sections[tab]); right.append(panel);
      button.addEventListener("click", () => { state.tab = tab; render(); save(); });
      button.addEventListener("keydown", (event) => {
        let index = tabs.indexOf(state.tab);
        if (event.key === "ArrowRight") index = (index + 1) % tabs.length;
        else if (event.key === "ArrowLeft") index = (index + tabs.length - 1) % tabs.length;
        else if (event.key === "Home") index = 0;
        else if (event.key === "End") index = tabs.length - 1;
        else return;
        event.preventDefault(); state.tab = tabs[index]; render(); save(); $("#tab-" + state.tab).focus();
      });
    }
    const sectionNavs = new Map();
    const inspectorHeader = document.createElement("div"); inspectorHeader.className = "inspector-navigation";
    tablist.before(inspectorHeader); inspectorHeader.append(tablist);
    // Move existing controls only; expansion is ephemeral UI state.
    function addNavigation(container, side, tab, nodes) {
      const nav = document.createElement("nav"); nav.className = "section-navigation";
      nav.setAttribute("aria-label", (tab || "Workspace") + " sections");
      const items = nodes.map(([id, node]) => {
        if (node.id) id = node.id; else node.id = id;
        const heading = node.querySelector("h2, h3, summary");
        node.prepend(heading);
        const toggle = document.createElement("button"); toggle.type = "button"; toggle.className = "section-toggle"; toggle.textContent = heading.textContent;
        const content = document.createElement("div"); content.id = id + "-content";
        for (const child of [...node.childNodes]) if (child !== heading) content.append(child);
        heading.replaceChildren(toggle); node.append(content);
        toggle.setAttribute("aria-controls", content.id);
        function expand(value) { if (node.tagName === "DETAILS") node.open = true; content.hidden = !value; toggle.setAttribute("aria-expanded", String(value)); }
        expand(true);
        toggle.addEventListener("click", event => { event.preventDefault(); expand(content.hidden); });
        const link = document.createElement("button"); link.type = "button"; link.textContent = toggle.textContent; link.dataset.section = id; link.setAttribute("aria-controls", id); nav.append(link);
        // Provider visibility remains owned by status.js, including legacy Model.
        link.hidden = node.hidden;
        new MutationObserver(() => { link.hidden = node.hidden; }).observe(node, { attributes: true, attributeFilter: ["hidden"] });
        link.addEventListener("click", () => {
          clearFocus(); state[side + "Collapsed"] = false;
          if (innerWidth < 960) state[(side === "left" ? "right" : "left") + "Collapsed"] = true;
          if (tab) state.tab = tab;
          expand(true); render(); save(); mark(id); toggle.focus({ preventScroll: true });
          const panel = side === "left" ? left : right;
          const sticky = side === "left" ? nav : inspectorHeader;
          panel.scrollTo({ top: Math.max(0, panel.scrollTop + node.getBoundingClientRect().top - sticky.getBoundingClientRect().bottom - 8), behavior: "instant" });
        });
        return { id, node, link, expand };
      });
      function mark(id) { for (const item of items) { item.link.setAttribute("aria-current", item.id === id ? "location" : "false"); item.node.classList.toggle("active-section", item.id === id); } }
      nav.addEventListener("keydown", event => {
        const index = items.findIndex(item => item.link === event.target); if (index < 0) return;
        const next = ["ArrowRight", "ArrowDown"].includes(event.key) ? (index + 1) % items.length : ["ArrowLeft", "ArrowUp"].includes(event.key) ? (index + items.length - 1) % items.length : event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : -1;
        if (next < 0) return; event.preventDefault(); items[next].link.focus({ preventScroll: true });
      });
      for (const [label, value] of [["Expand All", true], ["Collapse All", false]]) {
        const button = document.createElement("button"); button.type = "button"; button.textContent = label; button.dataset.expand = String(value);
        button.addEventListener("click", () => items.forEach(item => item.expand(value))); nav.append(button);
      }
      const panel = side === "left" ? left : right;
      panel.addEventListener("scroll", () => {
        if (container.hidden || panel.hidden) return;
        const edge = (side === "left" ? nav : inspectorHeader).getBoundingClientRect().bottom + 10;
        mark((items.filter(item => item.node.getBoundingClientRect().top <= edge).at(-1) || items[0]).id);
      }, { passive: true });
      mark(items[0].id); sectionNavs.set(tab || "workspace", nav); return nav;
    }
    history.open = true;
    left.prepend(addNavigation(left, "left", null, [["workspace-repositories", navigation], ["workspace-remembered", left.querySelector(".remembered-workspaces")], ["workspace-conversations", history]]));
    const changes = [...repo.querySelectorAll(":scope > h3")].map((heading, index) => {
      const node = document.createElement("section"); heading.before(node); node.append(heading);
      while (node.nextElementSibling && node.nextElementSibling.tagName !== "H3") node.append(node.nextElementSibling);
      return [["changes-status", "changes-worktree", "changes-staged", "changes-review"][index], node];
    });
    right.querySelector(".authority-review details").open = true;
    for (const tab of tabs) {
      const nodes = tab === "repository" ? changes : tab === "authority" ? [["tools-effective", right.querySelector("#effective-tools-title").closest("section")], ["tools-unavailable", right.querySelector("#unavailable-capabilities-title").closest("section")], ["tools-context", right.querySelector(".authority-review details")]] : sections[tab].map((node, index) => [tab + "-section-" + index, node]);
      inspectorHeader.append(addNavigation(right.querySelector("#panel-" + tab), "right", tab, nodes));
    }
    const notices = document.createElement("div"); notices.className = "shell-notices";
    notices.append($("#frontend-boot-status"), $("#backend-error"), connectionError); toolbar.after(notices);
    function save() { try { localStorage.setItem(key, JSON.stringify(state)); } catch { /* Layout remains usable without storage. */ } }
    const splitters = {};
    function splitter(side, panel) {
      const handle = document.createElement("div"); handle.id = side + "-splitter"; handle.className = "splitter"; handle.tabIndex = 0;
      handle.setAttribute("role", "separator"); handle.setAttribute("aria-orientation", "vertical"); handle.setAttribute("aria-label", "Resize " + (side === "left" ? "Workspace" : "Inspector")); handle.setAttribute("aria-controls", panel.id);
      let drag = null;
      function resize(value) { state[side] = bounds(side, value, shell.clientWidth, side === "left" ? (state.rightCollapsed ? 0 : right.getBoundingClientRect().width) : (state.leftCollapsed ? 0 : left.getBoundingClientRect().width)); render(); }
      handle.addEventListener("pointerdown", (event) => { if (event.button !== 0) return; event.preventDefault(); drag = { id: event.pointerId, x: event.clientX, width: panel.getBoundingClientRect().width }; handle.setPointerCapture(event.pointerId); handle.classList.add("dragging"); });
      handle.addEventListener("pointermove", (event) => { if (drag && drag.id === event.pointerId) resize(drag.width + (event.clientX - drag.x) * (side === "left" ? 1 : -1)); });
      function finish(event) { if (!drag || drag.id !== event.pointerId) return; drag = null; handle.classList.remove("dragging"); if (handle.hasPointerCapture(event.pointerId)) handle.releasePointerCapture(event.pointerId); save(); }
      for (const event of ["pointerup", "pointercancel", "lostpointercapture"]) handle.addEventListener(event, finish);
      handle.addEventListener("keydown", (event) => { if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return; event.preventDefault(); resize(event.key === "Home" ? 0 : event.key === "End" ? 10000 : panel.getBoundingClientRect().width + (event.key === "ArrowRight" ? 1 : -1) * (side === "left" ? 1 : -1) * (event.shiftKey ? 40 : 10)); save(); });
      splitters[side] = handle; return handle;
    }
    shell.replaceChildren(left, splitter("left", left), center, splitter("right", right), right);
    let wasNarrow = innerWidth < 960;
    let focusRestore = null;
    function clearFocus() {
      focusRestore = null;
      $("#focus-chat").textContent = "Focus Chat";
      $("#focus-chat").setAttribute("aria-pressed", "false");
    }
    function render() {
      for (const [tab, nav] of sectionNavs) if (tab !== "workspace") nav.hidden = tab !== state.tab;
      const narrow = window.innerWidth < 960;
      if (narrow && !wasNarrow) state.leftCollapsed = state.rightCollapsed = true;
      wasNarrow = narrow;
      shell.classList.toggle("narrow", narrow);
      let l = state.leftCollapsed ? 0 : state.left;
      let r = state.rightCollapsed ? 0 : state.right;
      if (!narrow) { r = Math.min(r, Math.max(260, shell.clientWidth - l - 332)); l = Math.min(l, Math.max(180, shell.clientWidth - r - 332)); }
      shell.style.setProperty("--left-width", l + "px"); shell.style.setProperty("--right-width", r + "px");
      for (const side of ["left", "right"]) {
        const collapsed = state[side + "Collapsed"];
        const panel = side === "left" ? left : right;
        panel.hidden = collapsed;
        splitters[side].hidden = collapsed || narrow;
        splitters[side].setAttribute("aria-valuemin", side === "left" ? "180" : "260");
        splitters[side].setAttribute("aria-valuemax", String(bounds(side, 10000, shell.clientWidth, side === "left" ? r : l)));
        splitters[side].setAttribute("aria-valuenow", String(Math.round(side === "left" ? l : r)));
        $(side === "left" ? "#toggle-workspace" : "#toggle-inspector").setAttribute("aria-expanded", String(!collapsed));
      }
      for (const tab of tabs) { const selected = tab === state.tab; $("#tab-" + tab).setAttribute("aria-selected", String(selected)); $("#tab-" + tab).tabIndex = selected ? 0 : -1; $("#panel-" + tab).hidden = !selected; }
    }
    for (const side of ["left", "right"]) $(side === "left" ? "#toggle-workspace" : "#toggle-inspector").addEventListener("click", () => { clearFocus(); state[side + "Collapsed"] = !state[side + "Collapsed"]; if (innerWidth < 960 && !state[side + "Collapsed"]) state[(side === "left" ? "right" : "left") + "Collapsed"] = true; render(); save(); });
    center.tabIndex = -1;
    clearFocus();
    $("#focus-chat").addEventListener("click", () => {
      if (focusRestore) {
        Object.assign(state, focusRestore);
        if (innerWidth < 960) state.leftCollapsed = state.rightCollapsed = true;
        clearFocus();
      } else {
        focusRestore = { leftCollapsed: state.leftCollapsed, rightCollapsed: state.rightCollapsed };
        state.leftCollapsed = state.rightCollapsed = true;
        $("#focus-chat").textContent = "Exit Chat Focus";
        $("#focus-chat").setAttribute("aria-pressed", "true");
      }
      render(); save();
      ($("#chat-prompt").disabled ? center : $("#chat-prompt")).focus();
    });
    $("#reset-layout").addEventListener("click", () => { clearFocus(); state = defaults(); if (innerWidth < 960) state.leftCollapsed = state.rightCollapsed = true; render(); save(); });
    // Narrow-window drawers are opt-in on launch; stored desktop widths survive.
    if (innerWidth < 960) state.leftCollapsed = state.rightCollapsed = true;
    window.addEventListener("resize", render);
    function summary() {
      $("#layout-repository").textContent = $("#repository-path")?.textContent || "No repository";
      const provider = $("#native-provider").value;
      const identity = { openai: "Native OpenAI", llama_cpp: "Native llama.cpp", codex: "Optional Codex CLI" }[provider] || "Runtime unavailable";
      const connection = [...$("#runtime-status").querySelectorAll("dt")].find(e => e.textContent === "Connection")?.nextElementSibling?.textContent || "not connected";
      const model = provider === "codex" ? $("#model-identifier").value : $("#native-model").value;
      $("#layout-runtime").textContent = `${identity} · ${model || "Provider default model"} · ${connection}`;
      $("#layout-runtime").title = $("#layout-runtime").textContent;
    }
    new MutationObserver(summary).observe($("#runtime-status"), { subtree: true, childList: true, characterData: true });
    new MutationObserver(summary).observe($("#repository-path"), { subtree: true, childList: true, characterData: true });
    render(); summary();
  }
  return { validate, bounds, defaults, install };
})();
if (typeof document !== "undefined") DesktopLayout.install();
if (typeof module !== "undefined") module.exports = DesktopLayout;
