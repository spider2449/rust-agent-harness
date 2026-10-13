// RAH Web UI settings: forms over the configuration of the built-in plugins
// (model, context, policy, loop). Each form starts from the plugin's current
// configuration, changes only its own fields and saves through the plugin
// management API, so options it does not show are kept. Other plugins stay
// in the Plugins panel's JSON editor.
"use strict";

const MODEL_KINDS = {
  "model-llamacpp": "llama.cpp (local server)",
  "model-openai": "OpenAI",
  "model-anthropic": "Anthropic (Claude)",
  "model-mock": "Mock (for testing)",
};
const LOOP_KINDS = {
  "loop-react": "Standard: answer and use tools step by step",
  "loop-plan": "Plan first, then carry out each step",
};
const EFFORTS = {
  "": "Model default",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "Extra high",
  max: "Max",
};
const LEVELS = ["none", "read", "write", "execute"];

const settings = { tab: "model", plugins: [], tools: [], status: null };

function field(label, input, hint) {
  const row = el("label", "field");
  row.append(el("span", "label", label), input);
  if (hint) row.append(el("small", "", hint));
  return row;
}

function textInput(value, placeholder) {
  const input = el("input");
  input.value = value ?? "";
  if (placeholder) input.placeholder = placeholder;
  input.spellcheck = false;
  return input;
}

function numberInput(value, placeholder) {
  const input = textInput(value, placeholder);
  input.type = "number";
  input.min = "1";
  return input;
}

function select(options, value) {
  const node = el("select");
  for (const [key, label] of Object.entries(options)) {
    const option = el("option", "", label);
    option.value = key;
    node.append(option);
  }
  node.value = value;
  return node;
}

function settingsMessage(text, bad) {
  const node = $("settings-message");
  node.textContent = text || "";
  node.className = `settings-message${bad ? " bad" : ""}`;
  node.hidden = !text;
}

// Sets `key` when `value` is non-empty, removes it otherwise.
function put(config, key, value) {
  if (value === "" || value === null || value === undefined || Number.isNaN(value)) delete config[key];
  else config[key] = value;
}

function numberOrEmpty(input) {
  return input.value.trim() === "" ? "" : Number(input.value);
}

function pluginOfKinds(kinds, preferred) {
  const candidates = settings.plugins.filter((plugin) => kinds.includes(plugin.kind));
  return candidates.find((plugin) => plugin.id === preferred) || candidates.find((p) => p.state === "active") || candidates[0];
}

// Saves `config` for `plugin`, or replaces it with `kind` when that differs.
// `done` is the message shown once the plugin is active again.
async function savePlugin(plugin, kind, config, label, done) {
  let result;
  if (plugin && plugin.kind === kind) {
    result = await api(`/plugins/${encodeURIComponent(plugin.id)}/config`, { method: "PUT", body: JSON.stringify({ config }) });
  } else if (plugin) {
    result = await api(`/plugins/${encodeURIComponent(plugin.id)}/replace`, {
      method: "POST",
      body: JSON.stringify({ kind, config }),
    });
  } else {
    result = await api("/plugins", { method: "POST", body: JSON.stringify({ kind, config }) });
  }
  await reloadSettings();
  if (result.state && result.state !== "active") {
    settingsMessage(`${label} saved, but ${result.id} is ${result.state}${result.detail ? `: ${result.detail}` : ""}`, true);
  } else {
    settingsMessage(`${label} saved.${done ? ` ${done(result.id)}` : ""}`);
    // Also under the conversation, where it stays after the dialog closes.
    if (done) showInfo(done(result.id));
  }
  loadPlugins().catch(() => {});
  refreshStatus();
}

function saveButton(label, handler) {
  const button = el("button", "", label);
  button.type = "button";
  button.addEventListener("click", async () => {
    button.disabled = true;
    settingsMessage("");
    try {
      await handler();
    } catch (error) {
      settingsMessage(error.message, true);
    } finally {
      button.disabled = false;
    }
  });
  const row = el("div", "buttons");
  row.append(button);
  return row;
}

// ---- Model ----------------------------------------------------------------

function modelForm() {
  const current = pluginOfKinds(Object.keys(MODEL_KINDS), settings.status?.model?.plugin);
  const config = { ...(current?.config && typeof current.config === "object" ? current.config : {}) };
  const form = el("div", "form");
  const kind = select(MODEL_KINDS, current?.kind || "model-llamacpp");
  form.append(field("Model provider", kind));
  const fields = el("div", "form");
  form.append(fields);
  const inputs = {};
  const render = () => {
    const same = kind.value === current?.kind;
    const value = (key) => (same ? config[key] : undefined);
    fields.replaceChildren();
    if (kind.value === "model-llamacpp") {
      inputs.endpoint = textInput(value("endpoint"), "http://127.0.0.1:8080");
      inputs.model = textInput(value("model"), "the model the server runs");
      fields.append(
        field("Server address", inputs.endpoint, "llama-server's address; loopback only."),
        field("Model name", inputs.model, "Leave empty when the server runs one model."),
      );
    } else if (kind.value === "model-openai") {
      inputs.model = textInput(value("model"), "for example gpt-5-mini");
      inputs.api_key_env = textInput(value("api_key_env"), "OPENAI_API_KEY");
      inputs.endpoint = textInput(value("endpoint"), "https://api.openai.com/v1/responses");
      fields.append(
        field("Model", inputs.model, "Required."),
        field("API key variable", inputs.api_key_env, "The name of the environment variable that holds the key, read from rah's environment. The key itself is never stored here."),
        field("Endpoint", inputs.endpoint, "Optional."),
      );
    } else if (kind.value === "model-anthropic") {
      inputs.model = textInput(value("model"), "claude-opus-5-5");
      inputs.api_key_env = textInput(value("api_key_env"), "ANTHROPIC_API_KEY");
      inputs.effort = select(EFFORTS, value("effort") || "");
      inputs.endpoint = textInput(value("endpoint"), "https://api.anthropic.com/v1/messages");
      fields.append(
        field("Model", inputs.model, "Leave empty for claude-opus-5-5."),
        field("API key variable", inputs.api_key_env, "The name of the environment variable that holds the key, read from rah's environment. The key itself is never stored here."),
        field("Effort", inputs.effort, "How much the model thinks before answering; higher is slower and costs more."),
        field("Endpoint", inputs.endpoint, "Optional."),
      );
    } else {
      inputs.label = textInput(value("label"), "model-mock");
      fields.append(field("Label", inputs.label, "Shown in the mock model's replies."));
    }
  };
  kind.addEventListener("change", render);
  render();
  form.append(
    saveButton("Save model", async () => {
      const next = kind.value === current?.kind ? { ...config } : {};
      for (const [key, input] of Object.entries(inputs)) {
        if (input.isConnected) put(next, key, input.value.trim());
      }
      if (kind.value === "model-openai" && !next.model) throw new Error("Enter the OpenAI model.");
      // The provider connects lazily, so the next message is the first
      // request to the new address or model.
      const model = next.model ? ` · ${next.model}` : "";
      await savePlugin(current, kind.value, next, "Model", (id) =>
        `Reconnected: ${MODEL_KINDS[kind.value]} as ${id}${model}. The next message uses it.`);
    }),
  );
  return form;
}

// ---- Assistant (context-default) ------------------------------------------

function assistantForm() {
  const plugin = pluginOfKinds(["context-default"]);
  const form = el("div", "form");
  if (!plugin) {
    form.append(el("p", "hint", "No context-default plugin is mounted, so there is no system prompt to set."));
    return form;
  }
  const config = { ...(plugin.config && typeof plugin.config === "object" ? plugin.config : {}) };
  const system = el("textarea");
  system.rows = 8;
  system.value = config.system || "";
  system.placeholder = "Instructions the model gets before every conversation, for example: Answer in Traditional Chinese. Be concise.";
  const maxItems = numberInput(config.max_items, "200");
  const maxChars = numberInput(config.max_chars, "200000");
  form.append(
    field("System prompt", system),
    field("History: messages kept", maxItems, "Older messages are left out of model requests; the session keeps them."),
    field("History: characters kept", maxChars),
    saveButton("Save assistant", async () => {
      const next = { ...config };
      put(next, "system", system.value.trim());
      put(next, "max_items", numberOrEmpty(maxItems));
      put(next, "max_chars", numberOrEmpty(maxChars));
      await savePlugin(plugin, plugin.kind, next, "Assistant");
    }),
  );
  return form;
}

// ---- Tools (policy-authority) ---------------------------------------------

// The policy's own matching: exact names win, then the longest prefix.
function matchPattern(map, name) {
  if (map && Object.prototype.hasOwnProperty.call(map, name)) return map[name];
  let best = null;
  for (const [pattern, value] of Object.entries(map || {})) {
    if (!pattern.endsWith("*")) continue;
    const prefix = pattern.slice(0, -1);
    if (name.startsWith(prefix) && (!best || prefix.length > best[0].length)) best = [prefix, value];
  }
  return best ? best[1] : undefined;
}

function levelDecision(policy, tool) {
  const assigned = matchPattern(policy.tools, tool.name);
  if (!assigned) return "deny";
  const effective = LEVELS[Math.max(LEVELS.indexOf(assigned), LEVELS.indexOf(tool.permission))];
  if ((policy.allow || ["none", "read"]).includes(effective)) return "allow";
  if ((policy.ask || ["write", "execute"]).includes(effective)) return "ask";
  return "deny";
}

const DECISION_LABELS = { allow: "allowed", ask: "asks first", deny: "off" };

function groupDecision(policy, levels) {
  const allow = policy.allow || ["none", "read"];
  const ask = policy.ask || ["write", "execute"];
  if (levels.every((level) => allow.includes(level))) return "allow";
  if (levels.every((level) => ask.includes(level))) return "ask";
  return "deny";
}

function toolsForm() {
  const plugin = pluginOfKinds(["policy-authority"]);
  const form = el("div", "form");
  if (!plugin) {
    form.append(el("p", "settings-message bad", "No policy plugin is mounted: every tool call runs without checks. Mount policy-authority in the Plugins panel to control tools here."));
    return form;
  }
  const policy = { ...(plugin.config && typeof plugin.config === "object" ? plugin.config : {}) };
  const reading = select({ allow: "Allow", ask: "Ask first", deny: "Off" }, groupDecision(policy, ["none", "read"]));
  const writing = select({ ask: "Ask first", allow: "Allow without asking", deny: "Off" }, groupDecision(policy, ["write", "execute"]));
  form.append(
    el("p", "hint", "Tools the model can use. “Default” follows the two rules below; a choice per tool overrides them."),
    field("Reading tools (files, Git status, search)", reading),
    field("Writing tools and commands (edit files, run programs)", writing, "Allowing these without asking lets the model change files and run programs on its own."),
  );
  const table = el("table", "tool-table");
  const head = el("tr");
  head.append(el("th", "", "Tool"), el("th", "", "Kind"), el("th", "", "Choice"));
  table.append(head);
  const choices = [];
  const preview = () => {
    const draft = rulesFrom(policy, reading.value, writing.value);
    for (const { tool, choice } of choices) {
      choice.options[0].textContent = `Default (${DECISION_LABELS[levelDecision(draft, tool)]})`;
    }
  };
  for (const tool of [...settings.tools].sort((a, b) => a.name.localeCompare(b.name))) {
    const row = el("tr");
    const name = el("td", "", tool.name);
    if (tool.description) {
      const description = tool.description.split("\n")[0];
      name.append(el("small", "description", description.length > 120 ? `${description.slice(0, 117)}…` : description));
      name.title = tool.description;
    }
    const exact = policy.decisions && Object.prototype.hasOwnProperty.call(policy.decisions, tool.name) ? policy.decisions[tool.name] : "default";
    const choice = select({ default: "Default", allow: "Allow", ask: "Ask first", deny: "Off" }, exact);
    choice.setAttribute("aria-label", `Choice for ${tool.name}`);
    choice.dataset.tool = tool.name;
    choices.push({ tool, choice });
    row.append(name, el("td", `level level-${tool.permission}`, tool.permission), el("td", "", ""));
    row.lastChild.append(choice);
    table.append(row);
  }
  reading.addEventListener("change", preview);
  writing.addEventListener("change", preview);
  preview();
  form.append(
    table,
    saveButton("Save tools", async () => {
      const next = rulesFrom(policy, reading.value, writing.value);
      // Keep decisions for patterns and for tools not offered right now.
      const decisions = { ...(policy.decisions || {}) };
      for (const { tool, choice } of choices) {
        if (choice.value === "default") delete decisions[tool.name];
        else decisions[tool.name] = choice.value;
      }
      put(next, "decisions", Object.keys(decisions).length ? decisions : "");
      if (writing.value === "allow" && !window.confirm("Let the model edit files and run programs without asking?")) return;
      await savePlugin(plugin, plugin.kind, next, "Tools");
    }),
  );
  return form;
}

// The policy with `allow` and `ask` set from the two group choices.
function rulesFrom(policy, reading, writing) {
  const next = { ...policy, allow: [], ask: [] };
  for (const [levels, choice] of [[["none", "read"], reading], [["write", "execute"], writing]]) {
    if (choice === "allow") next.allow.push(...levels);
    else if (choice === "ask") next.ask.push(...levels);
  }
  return next;
}

// ---- Loop -----------------------------------------------------------------

function loopForm() {
  const current = pluginOfKinds(Object.keys(LOOP_KINDS), settings.status?.loop?.plugin);
  const config = { ...(current?.config && typeof current.config === "object" ? current.config : {}) };
  const form = el("div", "form");
  const kind = select(LOOP_KINDS, current?.kind || "loop-react");
  form.append(field("How the assistant works", kind));
  const fields = el("div", "form");
  form.append(fields);
  const inputs = {};
  const render = () => {
    const same = kind.value === current?.kind;
    const value = (key) => (same ? config[key] : undefined);
    fields.replaceChildren();
    for (const key of Object.keys(inputs)) delete inputs[key];
    if (kind.value === "loop-react") {
      inputs.max_rounds = numberInput(value("max_rounds"), "32");
      fields.append(field("Most model rounds per message", inputs.max_rounds, "Rounds that may call tools. When they are used up, the model answers from what it has gathered."));
    } else {
      inputs.max_steps = numberInput(value("max_steps"), "8");
      inputs.max_rounds_per_step = numberInput(value("max_rounds_per_step"), "16");
      fields.append(
        field("Most plan steps", inputs.max_steps),
        field("Most model rounds per step", inputs.max_rounds_per_step, "Rounds that may call tools. When they are used up, the step ends with what it has gathered."),
      );
    }
  };
  kind.addEventListener("change", render);
  render();
  form.append(
    saveButton("Save loop", async () => {
      const next = kind.value === current?.kind ? { ...config } : {};
      for (const [key, input] of Object.entries(inputs)) put(next, key, numberOrEmpty(input));
      await savePlugin(current, kind.value, next, "Loop", (id) => `The next message uses ${id}.`);
    }),
  );
  return form;
}

// ---- Dialog ---------------------------------------------------------------

const FORMS = { model: modelForm, assistant: assistantForm, tools: toolsForm, loop: loopForm };

function renderSettings() {
  for (const button of document.querySelectorAll("#settings-tabs button")) {
    button.classList.toggle("current", button.dataset.tab === settings.tab);
    button.setAttribute("aria-selected", String(button.dataset.tab === settings.tab));
  }
  $("settings-body").replaceChildren(FORMS[settings.tab]());
}

async function reloadSettings() {
  const [plugins, info, status] = await Promise.all([api("/plugins"), api("/info"), api("/status").catch(() => null)]);
  settings.plugins = plugins;
  settings.tools = info.tools;
  settings.status = status;
  renderSettings();
}

async function openSettings(tab) {
  if (tab) settings.tab = tab;
  settingsMessage("");
  $("settings-dialog").showModal();
  try {
    await reloadSettings();
  } catch (error) {
    settingsMessage(error.message, true);
  }
}

function setupSettings() {
  $("open-settings").addEventListener("click", () => openSettings());
  $("settings-close").addEventListener("click", () => $("settings-dialog").close());
  for (const button of document.querySelectorAll("#settings-tabs button")) {
    button.addEventListener("click", () => {
      settings.tab = button.dataset.tab;
      settingsMessage("");
      renderSettings();
    });
  }
}
