import { Plugin } from "./rah_plugin.mjs";

const plugin = new Plugin(import.meta.url);
let count = 0;
plugin.view("board", () => [
  { type: "heading", text: "Tally board" },
  { type: "text", text: `Count: ${count}` },
  { type: "text", text: "A test counter. Restarting this plugin resets it." },
  { type: "button", action: "increment", label: "Add one" },
  { type: "button", action: "reset", label: "Reset count" },
]);
plugin.action("board", "increment", async (_input, ctx) => {
  count += 1;
  ctx.viewChanged("board");
  return { message: "Count increased.", rerender: true };
});
plugin.action("board", "reset", async (_input, ctx) => {
  count = 0;
  ctx.viewChanged("board");
  return { message: "Count reset.", rerender: true };
});
plugin.run();
