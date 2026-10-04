"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const elements = new Map();
const element = selector => {
  if (!elements.has(selector)) elements.set(selector, { value: "", children: [], replaceChildren(...children) { this.children = children; }, addEventListener() {}, querySelectorAll: () => [] });
  return elements.get(selector);
};
const context = vm.createContext({ document: { querySelector: element, createElement: () => ({}) }, chatRunning: false,
  renderedCodexStatus: "not connected", renderedModelSource: null, modelSourceRequest: 0, modelDraftDirty: false,
  renderedModelConfiguration: null, modelHint: value => value,
  renderModelPreflight: () => { context.cleared = true; } });
vm.runInContext(source.slice(source.indexOf("function renderModelConfiguration"), source.indexOf("function renderRepositorySnapshot")), context);
const preference = { provider: "openai", model: "gpt-6.1-sol", readiness: "not_tested" };
const render = (runtimeAdapter, currentModel) => context.renderModelConfiguration({ ...preference,
  runtimeSelection: { runtimeAdapter, currentModel, modelValidated: false } });
render("codex", "gpt-6.1-sol");
assert.equal(element("#model-provider").value, "openai");
render("openai", "configured-model");
assert.equal(element("#model-provider").value, "");
assert.equal(element("#model-provider").disabled, true);
assert.equal(element("#model-identifier").value, "configured-model");
assert.equal(element("#apply-model-configuration").disabled, true);
render("none", null);
assert.equal(element("#model-identifier").value, "");
assert.equal(element("#reset-model-preferences").disabled, true);
render("codex", "gpt-6.1-sol");
assert.equal(element("#model-provider").value, "openai");
assert.equal(element("#model-identifier").value, "gpt-6.1-sol");
const handlerStart = source.indexOf('document.querySelector("#model-provider").addEventListener("change"');
const handlerEnd = source.indexOf('document.querySelector("#apply-model-configuration").addEventListener', handlerStart);
let handler;
element("#model-provider").addEventListener = (_, callback) => { handler = callback; };
vm.runInContext(source.slice(handlerStart, handlerEnd), context);
for (const [from, to] of [["openai", "ollama"], ["ollama", "openai"], ["openai", "inherit"]]) {
  element("#model-provider").value = from;
  element("#model-identifier").value = "gpt-6.1-sol";
  element("#model-provider").value = to;
  handler();
  assert.equal(element("#model-identifier").value, "");
  assert.equal(context.cleared, true);
}
assert.equal(preference.model, "gpt-6.1-sol");
Object.assign(context, { renderRows: () => {}, applicationRows: [], runtimeRows: [],
  updateRepositorySelectionControls: () => {}, updateRepositoryCloseControls: () => {},
  renderedTrustedProfileSelection: null, resumeAvailable: false, resumeUsed: false });
vm.runInContext(source.slice(source.indexOf("async function loadStatus"), source.indexOf("function appendMessage")), context);
(async () => {
  for (const runtimeAdapter of ["openai", "none", "codex"]) {
    const snapshot = { adapter: runtimeAdapter, source: runtimeAdapter === "codex" ? { kind: "advertised_catalog", models: ["gpt-6-astra"] } : runtimeAdapter === "openai" ? { kind: "configured", model: "configured-model" } : { kind: "no_runtime" }, eligibility: runtimeAdapter === "codex" ? "advertised_absent" : runtimeAdapter === "openai" ? "configured" : "no_runtime" };
    await context.loadStatus(async command => command === "app_status" ? {
      runtimeAdapter, runtimeAvailable: runtimeAdapter !== "none", codexStatus: "not connected",
    } : { modelSource: snapshot, ...preference, modelSelectionMode: "advertised", runtimeSelection: { runtimeAdapter, currentModel: runtimeAdapter === "codex" ? preference.model : runtimeAdapter === "openai" ? "configured-model" : null } });
    assert.equal(element("#model-provider").disabled, runtimeAdapter !== "codex");
    assert.equal(element("#apply-model-configuration").disabled, runtimeAdapter !== "codex");
    assert.equal(element("#codex-connection").disabled, runtimeAdapter !== "openai");
    assert.equal(element("#new-conversation").disabled, false);
    assert.equal(element("#clear-conversation-history").disabled, false);
  }
  console.log("Task 509A frontend: adapter scoping, return to Codex, none Connect, and three stale-model fixtures passed");
})().catch(error => { console.error(error); process.exitCode = 1; });
