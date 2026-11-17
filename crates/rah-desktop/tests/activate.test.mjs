import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

test("native activation follows editor input and preserves transcript selection", () => {
  const sent = [];
  let listener;
  const context = {
    document: {
      hasFocus: () => true,
      addEventListener(type, handler, options) {
        assert.equal(type, "pointerdown");
        assert.equal(options.capture, true);
        listener = handler;
      },
    },
    window: { ipc: { postMessage: (message) => sent.push(message) } },
  };
  vm.runInNewContext(readFileSync(new URL("../src/activate.js", import.meta.url), "utf8"), context);
  const editor = { closest: () => ({}) };
  const transcript = { closest: () => null };
  listener({ isTrusted: false, button: 0, target: editor });
  listener({ isTrusted: true, button: 2, target: editor });
  listener({ isTrusted: true, button: 0, target: transcript });
  listener({ isTrusted: true, button: 0 });
  assert.deepEqual(sent, []);
  listener({ isTrusted: true, button: 0, target: editor });
  listener({ isTrusted: true, button: 0, target: editor });
  assert.deepEqual(sent, ["rah:activate", "rah:activate"]);
});
