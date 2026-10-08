"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const elements = new Map();
const element = id => {
  if (!elements.has(id)) elements.set(id, { value: "", options: ["openai", "llama_cpp", "codex"].map(value => ({ value })), handlers: {}, addEventListener(event, callback) { this.handlers[event] = callback; } });
  return elements.get(id);
};
const calls = [];
const context = vm.createContext({ document: { querySelector: element }, modelDraftDirty: false,
  refreshModelConfiguration: async () => {}, loadStatus: async () => {}, errorMessage: value => value });
vm.runInContext(source.slice(source.indexOf("async function initializeNativeConfiguration"), source.indexOf("async function initializeDesktop")), context);
(async () => {
  const invoke = async (name, args) => {
    calls.push({ name, args });
    if (name === "native_configuration") return { provider: "openai", model: null, endpoint: "http://127.0.0.1:8080", credentialConfigured: false, openaiAvailable: true, llamacppAvailable: true, codexAvailable: false };
  };
  await context.initializeNativeConfiguration(invoke);
  assert.equal(element("#native-provider").value, "openai");
  assert.equal(element("#native-provider").options[2].disabled, true);
  assert.match(element("#native-credential").textContent, /OPENAI_API_KEY.*RAH_LLAMA_CPP_API_KEY/);
  for (const provider of ["openai", "llama_cpp"]) {
    element("#native-provider").value = provider;
    element("#native-model").value = provider === "openai" ? "configured-model" : "";
    element("#native-model").handlers.input();
    assert.equal(element("#codex-connection").disabled, true);
    await element("#apply-native-configuration").handlers.click();
    const call = calls.at(-1);
    assert.equal(call.name, "set_native_configuration");
    assert.equal(call.args.provider, provider);
    assert.equal(call.args.model, provider === "openai" ? "configured-model" : null);
    assert.equal(call.args.endpoint, "http://127.0.0.1:8080");
    assert.equal(context.modelDraftDirty, false);
    assert.equal(Object.hasOwn(call.args, "key"), false);
  }
  assert.ok(!calls.some(call => /codex/.test(call.name)));
  console.log("Task 513 native provider configuration: PASS (2 providers, separate credential guidance, compiled availability)");
})().catch(error => { console.error(error); process.exitCode = 1; });
