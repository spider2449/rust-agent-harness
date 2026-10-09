"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const { spawnSync } = require("node:child_process");
const layout = require("./layout.js");
assert.deepEqual(layout.validate(null), layout.defaults());
for (const invalid of [{ ...layout.defaults(), left: -1 }, { ...layout.defaults(), version: 2 }, { ...layout.defaults(), tab: "private" }, { ...layout.defaults(), right: Infinity }]) assert.deepEqual(layout.validate(invalid), layout.defaults());
assert.equal(layout.bounds("left", -500, 1200, 340), 180);
assert.equal(layout.bounds("right", 10000, 1200, 240), 628);
assert.deepEqual(Object.keys(layout.validate({ ...layout.defaults(), repository: 'must not persist' })), Object.keys(layout.defaults()));
const edge = [process.env.PROGRAMFILES, process.env["PROGRAMFILES(X86)"]].map(p => path.join(p || "", "Microsoft/Edge/Application/msedge.exe")).find(fs.existsSync);
assert.ok(edge, "Edge required");
const temp = fs.mkdtempSync(path.join(os.tmpdir(), "rah-workspace-layout-"));
const original = fs.readFileSync(path.join(__dirname, "index.html"), "utf8");
const css = fs.readFileSync(path.join(__dirname, "styles.css"), "utf8");
const js = fs.readFileSync(path.join(__dirname, "layout.js"), "utf8");
const checks = `
try {
const check = (v, message) => { if (!v) throw Error(message); };
const $ = s => document.querySelector(s);
check($('#connection-error').parentElement === $('.shell-notices'), 'connection notice retained outside collapsible panels');
check([...$('.shell-notices').childNodes].every(node => node.nodeType !== Node.TEXT_NODE || !node.textContent.trim()), 'no literal null notice');
$('#connection-error').textContent = 'Runtime connection failed'; $('#connection-error').hidden = false;
check($('#connection-error').getBoundingClientRect().height > 0, 'connection rejection visible independently of Inspector');
$('#connection-error').hidden = true;
if (sessionStorage.getItem('reloading') === 'yes') {
 const restored = JSON.parse(localStorage.getItem('rah.desktop.layout.v1'));
 check(restored.tab === 'authority', 'tab persisted after reload');
 check($('#tab-authority').getAttribute('aria-selected') === 'true', 'tab restored after reload');
 if (innerWidth >= 960) check($('#workspace-panel').getBoundingClientRect().width === 250, 'width restored after reload');
 document.body.dataset.result = 'PASS';
} else {
check(document.documentElement.scrollWidth <= innerWidth, 'horizontal overflow');
const initial = $('#workspace-panel').getBoundingClientRect().width;
if (innerWidth >= 960) {
 $('#left-splitter').dispatchEvent(new KeyboardEvent('keydown', {key:'ArrowRight', bubbles:true}));
 check($('#workspace-panel').getBoundingClientRect().width === initial + 10, 'keyboard resize: ' + initial + ' -> ' + $('#workspace-panel').getBoundingClientRect().width + ', shell ' + $('.status-card').clientWidth);
 $('#right-splitter').dispatchEvent(new KeyboardEvent('keydown', {key:'Home', bubbles:true}));
 check($('#inspector-panel').getBoundingClientRect().width === 260, 'right lower bound');
 $('#left-splitter').dispatchEvent(new KeyboardEvent('keydown', {key:'End', bubbles:true}));
 check(document.documentElement.scrollWidth <= innerWidth, 'resize overflow');
 const width = $('#workspace-panel').getBoundingClientRect().width;
 $('#toggle-workspace').click(); check($('#workspace-panel').hidden, 'collapse');
 $('#toggle-workspace').click(); check($('#workspace-panel').getBoundingClientRect().width === width, 'restore');
 const saved = JSON.parse(localStorage.getItem('rah.desktop.layout.v1'));
 check(saved.left === width, 'persist width');
 // Synthetic events exercise the production drag handlers; capture is recorded
 // because synthetic pointers cannot acquire native browser capture.
 const handle = $('#left-splitter'); let captured = false;
 handle.setPointerCapture = () => { captured = true; };
 handle.hasPointerCapture = () => captured;
 handle.releasePointerCapture = () => { captured = false; };
 const pointer = (type,x) => handle.dispatchEvent(new PointerEvent(type,{pointerId:7,button:0,clientX:x,bubbles:true}));
 pointer('pointerdown', 200); check(captured && handle.classList.contains('dragging'), 'capture begins');
 pointer('pointermove', -10000); check($('#workspace-panel').getBoundingClientRect().width === 180, 'drag lower bound');
 pointer('pointercancel', -10000); check(!captured && !handle.classList.contains('dragging'), 'cancel releases capture');
 const stopped = $('#workspace-panel').getBoundingClientRect().width;
 pointer('pointermove', 10000); check($('#workspace-panel').getBoundingClientRect().width === stopped, 'inactive move does not resize');
 pointer('pointerdown', 200); pointer('pointermove',10000);
 check($('#workspace-panel').getBoundingClientRect().width <= 480 && document.documentElement.scrollWidth <= innerWidth, 'drag upper bound');
 pointer('pointerup',10000); check(!captured && !handle.classList.contains('dragging'), 'pointerup releases capture');
} else {
 check($('#workspace-panel').hidden && $('#inspector-panel').hidden, 'narrow chat priority');
 $('#toggle-inspector').click(); check(!$('#inspector-panel').hidden, 'narrow inspector accessible');
}
$('#tab-runtime').focus(); $('#tab-runtime').dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
check($('#tab-authority').getAttribute('aria-selected') === 'true', 'tab keyboard');
check(!$('#panel-authority').hidden, 'authority visible');
check($('#effective-tools') && $('#authorize-commit') && $('#repository-close-confirmation'), 'review parity');
check($('#codex-connection').closest('header'), 'connection toolbar');
// Section navigation exercises the actual production DOM and scroll containers.
const nav = $('#workspace-panel .section-navigation');
nav.querySelector('[data-expand="false"]').click();
check($('#workspace-remembered-content').hidden, 'collapse workspace sections');
const remembered = nav.querySelector('[data-section="workspace-remembered"]');
$('#toggle-workspace').click();
if (!$('#workspace-panel').hidden) $('#toggle-workspace').click();
const pageY = window.scrollY;
remembered.click();
check(!$('#workspace-panel').hidden && !$('#workspace-remembered-content').hidden, 'anchor restores panel and section');
check(window.scrollY === pageY, 'anchor does not scroll window');
check(document.activeElement === $('#workspace-remembered .section-toggle'), 'anchor focuses heading');
check(remembered.getAttribute('aria-current') === 'location', 'active anchor');
check($('#workspace-panel').scrollTop > 0, 'workspace scroll container');
remembered.focus(); remembered.dispatchEvent(new KeyboardEvent('keydown',{key:'End',bubbles:true}));
check(document.activeElement.dataset.section === 'workspace-conversations', 'anchor keyboard End');
nav.querySelector('[data-expand="true"]').click();
check(!$('#workspace-repositories-content').hidden, 'expand all workspace');
const staged = document.querySelector('[data-section="changes-staged"]');
const changesNav = staged.closest('nav');
changesNav.querySelector('[data-expand="false"]').click();
check($('#changes-staged-content').hidden && !$('#workspace-remembered-content').hidden, 'collapse all scoped to tab');
$('#focus-chat').click();
staged.click();
check(!$('#inspector-panel').hidden && $('#tab-repository').getAttribute('aria-selected') === 'true', 'anchor restores inspector and switches tab');
check(!$('#changes-staged-content').hidden && $('#changes-worktree-content').hidden, 'anchor expands only target');
check($('#focus-chat').getAttribute('aria-pressed') === 'false', 'navigation exits chat focus');
changesNav.querySelector('[data-expand="true"]').click();
check(!$('#changes-review-content').hidden, 'expand all changes');
$('#changes-review .section-toggle').click();
check($('#changes-review-content').hidden && !$('#changes-staged-content').hidden, 'independent collapse');
document.querySelector('[data-section="changes-review"]').click();
check(!$('#changes-review-content').hidden && $('#authorize-commit').closest('#changes-review-content'), 'existing commit control retained');
check($('#choose-repository').closest('#workspace-repositories-content'), 'existing repository control retained');
check(new Set([...document.querySelectorAll('[id]')].map(e=>e.id)).size === document.querySelectorAll('[id]').length, 'no duplicate IDs');
check(!location.hash, 'no navigation hash');
for(const dialog of document.querySelectorAll('dialog')) {
 dialog.showModal();
 const r=dialog.getBoundingClientRect(); check(r.left>=0 && r.right<=innerWidth && r.top>=0 && r.bottom<=innerHeight, 'dialog viewport bounds');
 for(const button of dialog.querySelectorAll('button')) { button.scrollIntoView({block:'nearest'}); const b=button.getBoundingClientRect(); check(b.left>=r.left && b.right<=r.right, 'dialog button horizontal clipping'); }
 dialog.close();
}
const beforeFocus = { left: $('#workspace-panel').hidden, right: $('#inspector-panel').hidden };
// The static shell starts disconnected; explicitly cover the enabled composer
// separately from the disabled-composer fallback below.
$('#chat-prompt').disabled = false;
$('#focus-chat').click(); check($('#workspace-panel').hidden && $('#inspector-panel').hidden, 'focus chat');
check(document.activeElement.id === 'chat-prompt', 'focus enabled composer');
check($('#focus-chat').getAttribute('aria-pressed') === 'true', 'focus toggle state');
$('#focus-chat').click();
check($('#focus-chat').getAttribute('aria-pressed') === 'false', 'exit focus toggle state');
if (innerWidth >= 960) check($('#workspace-panel').hidden === beforeFocus.left && $('#inspector-panel').hidden === beforeFocus.right, 'exit focus restores panels');
$('#chat-prompt').disabled = true;
$('#focus-chat').click(); check(document.activeElement.id === 'chat-panel', 'disconnected focus reaches chat');
$('#chat-prompt').disabled = false;
const composer = $('#chat-form').getBoundingClientRect(); check(composer.bottom <= innerHeight && composer.width > 0, 'composer visible');
$('#reset-layout').click(); check(JSON.parse(localStorage.getItem('rah.desktop.layout.v1')).left === 240, 'reset');
check(document.documentElement.scrollWidth <= innerWidth, 'final overflow');
if (innerWidth >= 960) $('#left-splitter').dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
$('#tab-authority').click();
sessionStorage.setItem('reloading', 'yes'); location.reload();
}
} catch(e) { document.body.dataset.result = e.message; }
`;
try {
 const file = path.join(temp, "layout.html");
 const corrupt = `if (!sessionStorage.getItem('reloading')) localStorage.setItem('rah.desktop.layout.v1', '{"version":1,"left":-1}');`;
 fs.writeFileSync(file, original.replace(/<link[^>]*href="styles.css"[^>]*>/, `<style>${css}</style>`).replace('<script src="layout.js"></script>', `<script>${corrupt}\n${js}</script>`).replace('<script src="status.js"></script>', `<script>${checks}</script>`));
 for (const width of [1600, 1200, 626, 476]) {
  const result = spawnSync(edge, ["--headless=new", "--disable-gpu", "--no-first-run", `--user-data-dir=${path.join(temp, 'profile-' + width)}`, `--window-size=${width},800`, "--virtual-time-budget=1000", "--dump-dom", `file:///${file.replace(/\\/g, '/')}`], { encoding: "utf8", timeout: 30000 });
  assert.equal(result.error, undefined, result.stderr);
  assert.equal(result.stdout.match(/data-result="([^"]*)"/)?.[1], 'PASS', `width ${width}: ${result.stderr.slice(-300)}`);
 }
 console.log("Workspace production layout: PASS (1600/1200/626/476, bounds, keyboard, collapse, tabs, persistence, reset, composer)");
} finally { fs.rmSync(temp, { recursive: true, force: true }); }
