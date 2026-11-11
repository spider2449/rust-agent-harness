import { Plugin, InvalidInput } from "./rah_plugin.mjs";

const plugin = new Plugin(import.meta.url);
plugin.tool("scratch-note.read", async (_args, ctx) => {
  const stored = await ctx.callHost("storage", "get", { key: "note" });
  return typeof stored.value === "string" ? stored.value : "No note saved.";
});
plugin.tool("scratch-note.write", async (args, ctx) => {
  if (typeof args?.text !== "string" || args.text.length > 2000) {
    throw new InvalidInput("text must be a string of at most 2000 characters");
  }
  await ctx.callHost("storage", "put", { key: "note", value: args.text });
  return { content: [{ type: "text", text: "Note saved." }], effect: "completed" };
});
plugin.run();
