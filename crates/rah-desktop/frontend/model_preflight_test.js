"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const source = fs.readFileSync(`${__dirname}/status.js`, "utf8");
const hint = { hidden: true, textContent: "" };
const alternatives = {
  children: [],
  replaceChildren() { this.children = []; },
  append(option) { this.children.push(option); },
};
const context = vm.createContext({
  document: {
    querySelector: selector => selector === "#model-preflight" ? hint : alternatives,
    createElement: () => ({}),
  },
  errorMessage: code => code,
});
vm.runInContext(source.slice(source.indexOf("function diagnosticText"), source.indexOf("function showChatError")), context);
context.renderModelPreflight({ outcome: "model_not_advertised", selectedModel: "missing", advertisedModels: ["alternative-a", "alternative-b"] });
assert.equal(hint.hidden, false);
assert.match(hint.textContent, /model_not_advertised.*Selected model: missing.*alternative-a, alternative-b/);
assert.deepEqual(alternatives.children.map(option => option.value), ["alternative-a", "alternative-b"]);
context.renderModelPreflight({ outcome: "model_advertised", selectedModel: "selected", advertisedModels: ["selected"] });
assert.match(hint.textContent, /does not prove entitlement or inference success/);
context.renderModelPreflight({ outcome: "not_checked", advertisedModels: [] });
assert.match(hint.textContent, /not checked/);
context.renderModelPreflight({ outcome: "model_catalog_unavailable", diagnostic: { operation: "model_discovery", kind: "provider_rejection", rpcCode: -32000, message: "SECRET_BODY", stderr: "SECRET_STDERR" } });
assert.match(hint.textContent, /model_catalog_unavailable.*runtime or provider request failed at model catalog.*-32000/);
assert.doesNotMatch(hint.textContent, /SECRET|authentication|outdated|retired/);
context.renderModelPreflight(null);
assert.equal(hint.hidden, true);
assert.equal(alternatives.children.length, 0);
console.log("model preflight frontend behavior passed");
