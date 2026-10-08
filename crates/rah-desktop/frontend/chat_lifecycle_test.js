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
  element("#chat-prompt").value = "first prompt";
  const context = vm.createContext({ document: { querySelector: element }, chatRunning: false, activeAssistant: null,
    appendMessage: () => ({ textContent: "" }), appendContextSeparator: () => {}, updateRepositoryMembershipControls: () => {},
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
(async () => {
  for (const kind of ["completed", "failed", "cancelled"]) {
    await scenario(kind, true);
    await scenario(kind, false);
  }
  await scenario("rejected", true, true);
  console.log("Desktop chat lifecycle: PASS (terminal before/after IPC reply and start rejection)");
})().catch(error => { console.error(error); process.exitCode = 1; });
