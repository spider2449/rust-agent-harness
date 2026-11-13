import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";

test("native activation follows real primary pointer input even with DOM focus", () => {
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
  listener({ isTrusted: false, button: 0 });
  listener({ isTrusted: true, button: 2 });
  assert.deepEqual(sent, []);
  listener({ isTrusted: true, button: 0 });
  listener({ isTrusted: true, button: 0 });
  assert.deepEqual(sent, ["rah:activate", "rah:activate"]);
});
