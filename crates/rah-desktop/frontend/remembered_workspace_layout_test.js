"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

// The production Desktop uses WebView2 on Windows. Exercise its Chromium layout
// engine with the real stylesheet and editor construction code.
const edge = [
  path.join(process.env.PROGRAMFILES || "", "Microsoft", "Edge", "Application", "msedge.exe"),
  path.join(process.env["PROGRAMFILES(X86)"] || "", "Microsoft", "Edge", "Application", "msedge.exe"),
].find((candidate) => fs.existsSync(candidate));
assert.ok(edge, "Microsoft Edge is required for the Windows Desktop layout test");

const source = fs.readFileSync(path.join(__dirname, "status.js"), "utf8");
const editorSource = source.slice(source.indexOf("function openRememberedEditor("), source.indexOf("function installRememberedWorkspaceHandlers("));
assert.ok(editorSource.startsWith("function openRememberedEditor("));
const css = fs.readFileSync(path.join(__dirname, "styles.css"), "utf8");
const shell = fs.readFileSync(path.join(__dirname, "index.html"), "utf8");
const temp = fs.mkdtempSync(path.join(os.tmpdir(), "rah-remembered-layout-"));

try {
  const script = `<script>
  ${editorSource}
  async function pickRememberedLocation() { return "F:\\\\fixture\\\\repository"; }
  function showRememberedError(error) { throw error; }
  function runRememberedMutation() {}
  const pageWidthBefore = document.documentElement.scrollWidth;
  openRememberedEditor(() => {}, {}, { candidateId: "fixture", label: "Example workspace", hasLocationHint: true });
  const editor = document.querySelector(".remembered-editor");
  const buttons = [...editor.querySelectorAll("button")];
  const rect = (element) => { const r = element.getBoundingClientRect(); return { left: r.left, right: r.right, top: r.top, bottom: r.bottom }; };
  const before = { viewport: [innerWidth, innerHeight], pageWidthBefore, pageWidth: document.documentElement.scrollWidth,
    dialog: rect(editor), buttons: buttons.map((button) => ({ name: button.textContent, rect: rect(button), disabled: button.disabled })) };
  editor.querySelector("button").click();
  setTimeout(() => {
    const afterChoose = { status: editor.querySelector("p").textContent, clearEnabled: !buttons[1].disabled };
    buttons.at(-1).scrollIntoView();
    const afterScroll = { dialog: rect(editor), save: rect(buttons.at(-2)), cancel: rect(buttons.at(-1)), scrollTop: editor.scrollTop };
    document.body.dataset.layoutResult = JSON.stringify({ before, afterChoose, afterScroll });
  }, 0);
  </script>`;
  const html = shell.replace(/<link[^>]*href="styles\.css"[^>]*>/, `<style>${css}</style>`)
    .replace('<script src="status.js"></script>', script);
  const file = path.join(temp, "layout.html");
  fs.writeFileSync(file, html);
  for (const [width, height] of [[626, 793], [506, 653], [476, 633]]) {
    const result = spawnSync(edge, ["--headless=new", "--disable-gpu", "--no-first-run", `--user-data-dir=${path.join(temp, `profile-${width}`)}`,
      `--window-size=${width},${height}`, "--virtual-time-budget=1000", "--dump-dom", `file:///${file.replace(/\\/g, "/")}`], { encoding: "utf8", timeout: 30000 });
    assert.equal(result.error, undefined, result.stderr);
    const match = result.stdout.match(/data-layout-result="([^"]+)"/);
    assert.ok(match, `layout result missing at ${width}x${height}: ${result.stderr}`);
    const data = JSON.parse(match[1].replace(/&quot;/g, '"').replace(/&amp;/g, "&"));
    const { before, afterScroll, afterChoose } = data;
    assert.ok(before.dialog.left >= 0 && before.dialog.right <= before.viewport[0], "dialog extends beyond viewport width");
    assert.ok(before.dialog.top >= 0 && before.dialog.bottom <= before.viewport[1], "dialog extends beyond viewport height");
    assert.ok(before.pageWidth <= before.pageWidthBefore, "editor increases page-level horizontal overflow");
    for (const button of before.buttons) {
      assert.ok(button.rect.left >= before.dialog.left && button.rect.right <= before.dialog.right, `${button.name} is clipped horizontally`);
    }
    assert.ok(before.buttons.some((button) => button.name === "Replace Location" && !button.disabled));
    assert.ok(before.buttons.some((button) => button.name === "Save Changes" && !button.disabled));
    assert.ok(before.buttons.some((button) => button.name === "Cancel" && !button.disabled));
    assert.equal(afterChoose.clearEnabled, true);
    assert.match(afterChoose.status, /New location selected/);
    for (const action of [afterScroll.save, afterScroll.cancel]) {
      assert.ok(action.top >= afterScroll.dialog.top && action.bottom <= afterScroll.dialog.bottom, "action is unreachable after scrolling");
    }
  }
  console.log("remembered workspace browser layout tests passed");
} finally {
  fs.rmSync(temp, { recursive: true, force: true });
}
