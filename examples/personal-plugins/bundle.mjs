// Build a plugin-builder import bundle without mounting or running any plugin.
import { readFile, writeFile } from "node:fs/promises";

const descriptions = {
  "text-lab": "Counts text without saving it. Tests a tool-only personal plugin.",
  "scratch-note": "Stores one short note. Tests declared storage access and read/write permissions.",
  "tally-board": "A counter page with buttons. Tests a personal UI view and its actions.",
};
const plugins = await Promise.all(Object.entries(descriptions).map(async ([name, description]) => ({
  name,
  description,
  runtime: "node",
  manifest: JSON.parse(await readFile(new URL(`${name}/manifest.json`, import.meta.url), "utf8")),
  files: [{ path: "plugin.mjs", content: await readFile(new URL(`${name}/plugin.mjs`, import.meta.url), "utf8") }],
})));
const bundle = { format: "rah-personal-plugins", version: 1, enabled: [], plugins };
if (!process.argv[2]) throw new Error("Usage: node examples/personal-plugins/bundle.mjs output.json");
await writeFile(process.argv[2], JSON.stringify(bundle, null, 2) + "\n", { flag: "wx" });
