"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const elements = new Map();
const element = selector => {
  if (!elements.has(selector)) elements.set(selector, { value: "", children: [], handlers: {},
    replaceChildren(...children) { this.children = children; },
    addEventListener(event, callback) { this.handlers[event] = callback; },
  });
  return elements.get(selector);
};
const context = vm.createContext({ document: { querySelector: element, createElement: () => ({}) },
  renderedModelConfiguration: null, renderedModelSource: null, renderedCodexStatus: "not connected",
  renderModelConfiguration: configuration => { context.renderedModelConfiguration = configuration; },
  chatRunning: false, modelDraftDirty: false, modelSourceRequest: 0 });
vm.runInContext(source.slice(source.indexOf("function renderModelSource"), source.indexOf("function renderRepositorySnapshot")), context);
const configuration = (mode, id = "gpt-6-astra") => ({ provider: "openai", model: id,
  modelSelectionMode: mode, runtimeSelection: { runtimeAdapter: "codex" } });
const snapshot = (kind = "advertised_catalog", eligibility = "advertised_unverified") => ({
  adapter: "codex", generation: 1, requestId: 1, source: { kind, models: ["gpt-6-astra", "gpt-6-luna"] }, eligibility,
});
const values = () => element("#model-picker").children.filter(o => !o.disabled).map(o => o.value);
context.renderedModelConfiguration = configuration("advertised");
context.renderModelSource(snapshot());
assert.deepEqual(values(), ["", "gpt-6-astra", "gpt-6-luna", "__custom__"]);
assert.equal(element("#model-identifier").hidden, true);
assert.equal(context.modelConnectAllowed(), true);
assert.match(element("#model-source-hint").textContent, /Advertised by Codex runtime.*Unverified/);
context.renderedModelConfiguration = configuration("advertised", "gpt-6.1-sol");
context.renderModelSource(snapshot("advertised_catalog", "advertised_absent"));
assert.ok(!values().includes("gpt-6.1-sol"));
assert.equal(context.modelConnectAllowed(), false);
assert.equal(element("#model-identifier").hidden, true);
assert.equal(context.renderedModelConfiguration.modelSelectionMode, "advertised");
context.renderedModelConfiguration = configuration("custom");
context.renderModelSource(snapshot("advertised_catalog", "custom_unverified"));
assert.equal(element("#model-picker").value, "__custom__");
assert.equal(element("#model-identifier").hidden, false);
assert.equal(context.renderedModelConfiguration.modelSelectionMode, "custom");
assert.match(element("#model-source-hint").textContent, /Custom · unverified/);
for (const eligibility of ["invalid_configuration", "loading", "known_rejection", "source_unavailable", "no_selection"]) {
  context.renderModelSource(snapshot("error", eligibility));
  assert.equal(context.modelConnectAllowed(), false);
}
context.renderedModelConfiguration = configuration("advertised");
for (const kind of ["loading", "empty_catalog", "error", "unavailable"]) {
  context.renderModelSource(snapshot(kind, "source_unavailable"));
  assert.ok(!values().includes("gpt-6-astra"));
  assert.equal(context.modelConnectAllowed(), false);
}
context.renderedModelConfiguration = { provider: "inherit", model: null, modelSelectionMode: null };
context.renderModelSource(snapshot("runtime_default", "inherited_unverified"));
assert.deepEqual(values(), [""]);
assert.equal(element("#model-picker").disabled, true);
assert.equal(context.modelConnectAllowed(), true);
context.renderedModelConfiguration = configuration("custom");
context.renderModelSource({ adapter: "openai", source: { kind: "configured", model: "native-configured" }, eligibility: "configured" });
assert.deepEqual(values(), ["native-configured"]);
assert.equal(element("#model-identifier").hidden, true);
assert.match(element("#model-source-hint").textContent, /Configured for native OpenAI/);
assert.equal(element("#model-picker").disabled, true);
context.renderModelSource({ adapter: "openai", source: { kind: "unavailable" }, eligibility: "source_unavailable" });
assert.ok(!values().includes("__custom__"));
assert.equal(context.modelConnectAllowed(), false);
context.renderModelSource({ adapter: "none", source: { kind: "no_runtime" }, eligibility: "no_runtime" });
assert.equal(context.modelConnectAllowed(), false);
const handlerStart = source.indexOf('  document.querySelector("#model-picker").addEventListener');
const handlerEnd = source.indexOf('  document.querySelector("#apply-model-configuration").addEventListener', handlerStart);
vm.runInContext(source.slice(handlerStart, handlerEnd), context);
element("#model-picker").value = "__custom__";
element("#model-picker").handlers.change();
assert.equal(element("#model-identifier").hidden, false);
assert.equal(context.modelDraftDirty, true);
assert.equal(element("#codex-connection").disabled, true);
element("#model-picker").value = "gpt-6-luna";
element("#model-picker").handlers.change();
assert.equal(element("#model-identifier").hidden, true);

(async () => {
  const deferred = () => { let resolve; let reject; const promise = new Promise((a,b) => { resolve=a; reject=b; }); return {promise, resolve, reject}; };
  for (const failure of [false, true]) {
    context.renderedModelConfiguration = configuration("advertised");
    const a = deferred(); const b = deferred();
    const old = context.refreshModelSource(() => a.promise);
    const newer = context.refreshModelSource(() => b.promise);
    b.resolve({ ...configuration("advertised"), modelSource: snapshot() }); await newer;
    const expected = context.renderedModelSource;
    if (failure) a.reject(new Error("late error")); else a.resolve({ ...configuration("advertised"), modelSource: snapshot("empty_catalog", "advertised_absent") });
    await old;
    assert.equal(context.renderedModelSource, expected);
    assert.equal(element("#model-picker").disabled, false);
  }
  context.renderedCodexStatus = "connected";
  context.renderModelSource(snapshot());
  assert.equal(element("#model-picker").disabled, true);
  assert.equal(element("#model-identifier").disabled, true);
  console.log("Task 509B-R2 picker: Advertised/Custom/Inherit/native/None, stale success/error/loading, explicit action and connected immutability passed");
})().catch(error => { console.error(error); process.exitCode = 1; });
