"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const elements = new Map();
const element = id => {
  if (!elements.has(id)) elements.set(id, { value: "prompt", hidden: true, textContent: "", handlers: {}, addEventListener(name, handler) { this.handlers[name] = handler; } });
  return elements.get(id);
};
async function scenario(kind, beforeReply, rejected = false) {
  const calls = [];
  const prompts = [];
  const messages = [];
  element("#chat-prompt").value = "first prompt";
  const context = vm.createContext({ document: { querySelector: element }, chatRunning: false, activeAssistant: null,
    appendMessage: (role, text) => { const message = { role, textContent: text }; messages.push(message); return message; }, appendContextSeparator: () => {}, updateRepositoryMembershipControls: () => {},
    errorMessage: () => "Chat could not finish.", diagnosticText: () => "",
    modelConnectAllowed: () => true, resumeAvailable: false, resumeUsed: false,
    showBackendError: () => {}, showChatError: () => {} });
  // Exercise the production control renderer, including Disconnect and native
  // provider configuration, with the state left by the actual submit handler.
  const controlsStart = source.indexOf('  const button = document.querySelector("#codex-connection");', source.indexOf("async function loadStatus("));
  const controlsEnd = source.indexOf('  document.querySelector("#new-conversation").disabled', controlsStart);
  vm.runInContext(`function renderControls(status) { const codex = false; ${source.slice(controlsStart, controlsEnd)} }`, context);
  context.loadStatus = async () => context.renderControls({ codexStatus: "connected", runtimeAvailable: true });
  const invoke = async (name, args) => {
    calls.push(name);
    if (name === "send_chat") {
      prompts.push(args.prompt);
      if (beforeReply && !rejected) context.handleChatEvent(invoke, { payload: { kind } });
      if (rejected) throw "chat_start_failed";
      return {};
    }
  };
  context.invoke = invoke;
  vm.runInContext(source.slice(source.indexOf("function handleChatEvent("), source.indexOf("async function toggleCodexConnection(")), context);
  vm.runInContext(source.slice(source.indexOf('  document.querySelector("#chat-form").addEventListener'), source.indexOf("  await refreshTrustedProfileSelection(invoke);", source.indexOf('  document.querySelector("#chat-form").addEventListener'))), context);
  const submit = element("#chat-form").handlers.submit;
  await submit({ preventDefault() {} });
  if (!beforeReply && !rejected) {
    assert.equal(context.chatRunning, true, "accepted active turn must retain UI ownership until terminal");
    context.handleChatEvent(invoke, { payload: { kind } });
  }
  assert.equal(context.chatRunning, false, `${kind}: terminal/rejection must leave UI idle (beforeReply=${beforeReply})`);
  await context.loadStatus();
  assert.equal(element("#codex-connection").disabled, false, "Disconnect must be enabled after terminal/rejection");
  assert.equal(element("#chat-prompt").disabled, false);
  assert.equal(element("#chat-send").textContent, "Send");
  for (const codexStatus of ["not connected", "error"]) {
    context.renderControls({ codexStatus, runtimeAvailable: true });
    for (const id of ["native-provider", "native-model", "native-endpoint", "apply-native-configuration"]) {
      assert.equal(element(`#${id}`).disabled, false, "disconnected/failed preflight must permit provider configuration");
    }
    assert.equal(element("#codex-connection").disabled, false);
  }
  element("#chat-prompt").value = "second prompt";
  await submit({ preventDefault() {} });
  assert.equal(calls.filter(name => name === "send_chat").length, 2, "second prompt must dispatch send_chat");
  assert.ok(!calls.includes("cancel_chat"), "second prompt must not cancel a completed turn");
  assert.deepEqual(prompts, ["first prompt", "second prompt"]);
}
async function failurePresentation(partial) {
  const messages = [];
  const context = vm.createContext({ document: { querySelector: element }, chatRunning: true, activeAssistant: null,
    appendMessage: (role, text) => { const message = { role, textContent: text }; messages.push(message); return message; },
    updateRepositoryMembershipControls: () => {}, loadStatus: async () => {}, showBackendError: () => {},
    errorMessage: () => "Chat could not finish." });
  vm.runInContext(source.slice(source.indexOf("function diagnosticText("), source.indexOf("function renderModelPreflight(")), context);
  vm.runInContext(source.slice(source.indexOf("function showChatError("), source.indexOf("async function toggleCodexConnection(")), context);
  const emit = payload => context.handleChatEvent(() => {}, { payload });
  emit({ kind: "started" });
  assert.equal(messages.length, 0, "Started must not render a blank response");
  emit({ kind: "delta", text: "" });
  assert.equal(messages.length, 0);
  if (partial) emit({ kind: "delta", text: "Partial response" });
  emit({ kind: "failed", code: "chat_runtime_failed", diagnostic: { operation: "turn", kind: "transport", source: "secret provider body" } });
  assert.equal(context.chatRunning, false);
  assert.equal(context.activeAssistant, null);
  assert.ok(messages.every(message => message.textContent.length));
  const failure = messages.at(-1).textContent;
  assert.match(failure, /Turn failed\./);
  assert.match(failure, /transport failure at chat turn/);
  assert.ok(!failure.includes("secret provider body"));
  assert.ok(!failure.includes("Completed"));
  if (partial) {
    assert.equal(messages[0].textContent, "Partial response");
    assert.match(failure, /response above is incomplete/);
  } else assert.match(failure, /No response text was received/);
  context.chatRunning = true;
  emit({ kind: "started" });
  emit({ kind: "delta", text: "Recovered" });
  emit({ kind: "completed" });
  assert.equal(messages.at(-1).textContent, "Recovered");
  assert.equal(context.chatRunning, false);
}
function activityClosure() {
  const entries = { children: [], append(entry) { this.children.push(entry); } };
  const context = vm.createContext({ maxActivityEntries: 100, document: {
    querySelector: () => entries,
    createElement: () => ({ dataset: {}, children: [], append(...children) { this.children.push(...children); }, querySelector() { return this.children[1]; } }),
  } });
  vm.runInContext(source.slice(source.indexOf("function appendActivity("), source.indexOf("function showBackendError(")), context);
  const emit = (activityId, kind, extra = {}) => context.appendActivity({ activityId, kind, tool: "repo.list", ...extra });
  emit("first", "tool_requested");
  emit("first", "tool_started");
  emit("second", "tool_requested");
  emit("second", "tool_started");
  emit("first", "tool_finished", { result: "success" });
  assert.equal(entries.children[1].children[1].textContent, "Started");
  assert.equal(entries.children[3].children[1].textContent, "Running", "another same-name call remains independent");
  emit("second", "tool_interrupted", { started: true });
  assert.equal(entries.children[3].children[1].textContent, "Outcome unknown");
  assert.equal(entries.children.at(-1).children[1].textContent, "Outcome unknown");
  assert.ok(!entries.children.some(entry => entry.children[1].textContent === "Running"));
  assert.equal(entries.children[4].children[1].textContent, "Completed", "confirmed result remains completed");
  emit("denied", "tool_requested");
  emit("denied", "tool_interrupted", { started: false });
  assert.equal(entries.children.at(-1).children[1].textContent, "Not started");
  assert.ok(!entries.children.at(-1).children[1].textContent.includes("Completed"));
}
(async () => {
  activityClosure();
  await failurePresentation(false);
  await failurePresentation(true);
  for (const kind of ["completed", "failed", "cancelled"]) {
    await scenario(kind, true);
    await scenario(kind, false);
  }
  await scenario("rejected", true, true);
  console.log("Desktop chat lifecycle: PASS (terminal before/after IPC reply and start rejection)");
})().catch(error => { console.error(error); process.exitCode = 1; });
