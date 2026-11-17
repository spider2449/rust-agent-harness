import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

const source = readFileSync(new URL("../../../plugins/frontend-web/assets/app.js", import.meta.url), "utf8");
const restore = source.slice(source.indexOf("function restoreComposerFocus()"), source.indexOf("async function start()"));

test("window focus preserves selected transcript text and restores an idle composer", () => {
  let focused = 0;
  let selection = { isCollapsed: false };
  const input = { disabled: false, focus: () => focused++ };
  const body = {};
  const context = vm.createContext({
    $: () => input,
    document: { activeElement: body, body, hidden: false, querySelector: () => null },
    window: { getSelection: () => selection },
  });
  vm.runInContext(restore, context);
  vm.runInContext("restoreComposerFocus()", context);
  assert.equal(focused, 0);
  selection = { isCollapsed: true };
  vm.runInContext("restoreComposerFocus()", context);
  assert.equal(focused, 1);
});
