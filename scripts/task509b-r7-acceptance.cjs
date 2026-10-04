"use strict";
// Actual-app acceptance: observe the normal frontend; never poll model_configuration.
// Launch the isolated Codex Desktop profile in Inherit before running this script.
const port = process.env.RAH_ACCEPTANCE_CDP_PORT || "9509";
(async () => {
  const pages = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const page = pages.find(p => p.type === "page" && /tauri|localhost/.test(p.url));
  if (!page) throw Error("RAH WebView page unavailable");
  const socket = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  try {
    const response = await new Promise(resolve => {
      socket.onmessage = event => { const value = JSON.parse(event.data); if (value.id === 1) resolve(value); };
      socket.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: {
        awaitPromise: true, returnByValue: true, expression: `
(async () => {
  const assert = (value, message) => { if (!value) throw Error(message); };
  const wait = async check => {
    const deadline = Date.now() + 60000;
    while (!check()) { if (Date.now() > deadline) throw Error("DOM acceptance timeout"); await new Promise(resolve => setTimeout(resolve, 100)); }
  };
  await wait(() => document.querySelector("#frontend-boot-status")?.textContent === "Desktop UI ready");
  assert(renderedModelSource.adapter === "codex", "Codex fixture required");
  assert(renderedModelSource.eligibility === "inherited_unverified", "Inherit fixture required");
  const before = JSON.parse(JSON.stringify(renderedModelSource));
  const model = before.source.models[0];
  assert(model, "advertised model required");
  const select = (selector, value) => { const element = document.querySelector(selector); element.value = value; element.dispatchEvent(new Event("change", { bubbles: true })); };
  select("#model-provider", "openai"); select("#model-picker", model);
  document.querySelector("#apply-model-configuration").click();
  await wait(() => renderedModelSource.eligibility === "advertised_unverified" && !document.querySelector("#codex-connection").disabled);
  assert(renderedModelSource.source.kind === "advertised_catalog", "resolved catalog required");
  assert(renderedModelSource.selectedModel === model && renderedModelSource.selectionMode === "advertised", "selection retained");
  assert(renderedModelSource.compatibility === "unverified", "compatibility must remain unverified");
  assert(renderedModelConfiguration.modelSource.requestId === renderedModelSource.requestId, "consumed/rendered request agrees");
  return { before, source: renderedModelSource, configuration: renderedModelConfiguration, hint: document.querySelector("#model-source-hint").textContent, connectDisabled: document.querySelector("#codex-connection").disabled };
})()`
      }}));
    });
    if (response.error || response.result?.exceptionDetails) throw Error(JSON.stringify(response));
    console.log(JSON.stringify(response.result.result.value, null, 2));
  } finally { socket.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
