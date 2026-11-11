import { Plugin, InvalidInput } from "./rah_plugin.mjs";

const plugin = new Plugin(import.meta.url);
plugin.tool("text-lab.inspect", async (args, ctx) => {
  if (typeof args?.text !== "string" || args.text.length > 10000) {
    throw new InvalidInput("text must be a string of at most 10000 characters");
  }
  return {
    label: typeof ctx.config?.label === "string" ? ctx.config.label : "Text inspection",
    characters: [...args.text].length,
    words: args.text.trim() ? args.text.trim().split(/\s+/u).length : 0,
  };
});
plugin.run();
