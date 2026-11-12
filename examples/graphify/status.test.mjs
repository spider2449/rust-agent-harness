import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';
import { StatusReporter, readStatus, hostStatusFile, MAX_STATUS_BYTES } from './status.mjs';
import { loadView, renderStatus } from './status-view.mjs';

const sample = fields => ({ version: 1, owner_pid: 42, publisher_pid: 100,
  workspace: 'workspace', state: 'ready', stage: 'Watching for changes', heartbeat_at: new Date(100000).toISOString(), ...fields });
const values = view => Object.fromEntries(view.nodes.flatMap(node => node.type === 'key_value' ? node.items.map(({ key, value }) => [key, value]) : []));
const options = { owner: 42, now: 100000, isAlive: () => true };

test('footer states reflect progress, failure and publisher health', () => {
  const indicator = (data, overrides = {}) => renderStatus(data, { ...options, statusBar: true, ...overrides }).nodes.find(node => node.type === 'status');
  assert.equal(indicator(null).state, 'idle');
  assert.equal(indicator(sample({})).state, 'ready');
  assert.deepEqual(indicator(sample({ state: 'building', progress: { completed: 1, total: 3 } })).label, 'Graphify 1/3');
  assert.equal(indicator(sample({ state: 'building' })).state, 'busy');
  assert.equal(indicator(sample({ state: 'error', error: 'failed' })).state, 'error');
  assert.match(indicator(sample({ state: 'error', error: 'failed' })).detail, /Last error: failed/);
  assert.equal(indicator(sample({}), { isAlive: () => false }).state, 'idle');
  assert.equal(indicator(sample({ heartbeat_at: new Date(0).toISOString() })).state, 'warning');
  assert.equal(indicator(sample({ state: '__proto__' })).state, 'warning');
  assert.ok([...indicator(sample({ workspace: 'x'.repeat(4096) })).detail].length <= 1024);
});

async function directory(t) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'rah-graphify-status-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  return root;
}

test('status artifacts publish ordered progress, success, failure and recovery', async t => {
  const root = await directory(t);
  const runtime = path.join(root, 'runtime');
  let now = 100000;
  const reporter = new StatusReporter(root, { directory: runtime, owner: 42, now: () => now });
  t.after(() => reporter.stop());
  await reporter.update({ state: 'building', started_at: new Date(now).toISOString() });
  const stages = [reporter.progress('found 277 code'), reporter.progress('AST extraction: 100/277 uncached files (36%)')];
  await Promise.all(stages);
  let status = await readStatus(hostStatusFile(runtime, 42));
  assert.equal(status.state, 'building');
  assert.deepEqual(status.progress, { completed: 100, total: 277 });
  assert.equal(values(renderStatus(status, options)).Files, '100 / 277');
  now += 1000;
  await reporter.update({ state: 'ready', node_count: 10, edge_count: 20, last_success_at: new Date(now).toISOString() });
  await reporter.update({ state: 'error', error: 'extract failed' });
  status = await readStatus(hostStatusFile(runtime, 42));
  assert.equal(status.state, 'error');
  assert.equal(status.node_count, 10);
  assert.equal(status.last_success_at, new Date(now).toISOString());
  assert.equal(values(renderStatus(status, { ...options, now })).Status, 'Update failed');
  await reporter.update({ state: 'ready', error: null });
  assert.equal((await readStatus(path.join(root, '.rah/graphify-out/status.json'))).error, null);
  await reporter.stop();
  assert.equal((await readStatus(hostStatusFile(runtime, 42))).state, 'stopped');
});

test('progress distinguishes parsed files from finished graph publication', async t => {
  const root = await directory(t);
  const reporter = new StatusReporter(root);
  await reporter.update({ state: 'building' });
  await reporter.progress('AST extraction: 277/277 uncached files (100%)');
  assert.equal(reporter.value.state, 'building');
  assert.equal(reporter.value.stage, 'Resolving relationships');
  await reporter.progress('introspecting Cargo workspace');
  assert.equal(reporter.value.stage, 'Resolving workspace dependencies');
});

test('missing feeds wait; dead and stale publishers are not shown as ready', () => {
  assert.equal(values(renderStatus(null, options)).Status, 'Waiting');
  assert.equal(values(renderStatus(sample({}), { ...options, isAlive: () => false })).Status, 'Stopped');
  assert.equal(values(renderStatus(sample({ heartbeat_at: new Date(0).toISOString() }), options)).Status, 'Not responding');
  assert.equal(values(renderStatus(sample({ state: 'stopped' }), options)).Status, 'Stopped');
  assert.throws(() => renderStatus(sample({ owner_pid: 99 }), options), /another RAH/);
});

test('status rendering bounds untrusted text and does not invent progress', () => {
  const view = renderStatus(sample({ progress: { completed: 100, total: 2 }, error: '<script>'.repeat(1000) }), options);
  const fields = values(view);
  assert.equal(fields.Files, '—');
  assert.ok(fields['Last error'].length <= 1500);
  assert.ok(view.nodes.every(node => ['heading', 'key_value', 'text'].includes(node.type)));
  assert.equal(values(renderStatus(sample({ state: '__proto__' }), options)).Status, 'Unknown');
});

test('elapsed time advances while building; last successful time is retained', () => {
  const data = sample({ state: 'building', started_at: new Date(90000).toISOString(), last_success_at: new Date(80000).toISOString() });
  assert.equal(values(renderStatus(data, options)).Elapsed, '10.0 s');
  assert.equal(values(renderStatus(data, { ...options, now: 101000 })).Elapsed, '11.0 s');
  assert.notEqual(values(renderStatus(data, options))['Last indexed'], '—');
});

test('malformed and oversized feeds produce an unavailable view', async t => {
  const root = await directory(t);
  const file = hostStatusFile(root, 42);
  await fs.writeFile(file, '{broken');
  assert.match(JSON.stringify(await loadView(root, options)), /Status unavailable/);
  assert.equal((await loadView(root, { ...options, statusBar: true })).nodes.at(-1).state, 'warning');
  await fs.writeFile(file, 'x'.repeat(MAX_STATUS_BYTES + 1));
  await assert.rejects(readStatus(file), /too large/);
  await fs.writeFile(file, JSON.stringify({ version: 2, state: 'ready', workspace: 'x' }));
  await assert.rejects(readStatus(file), /Unsupported/);
});

test('the process view advertises UiViews, not tools, and notifies live updates', { timeout: 10000 }, async t => {
  const root = await directory(t);
  const child = spawn(process.execPath, [fileURLToPath(new URL('./status-view.mjs', import.meta.url))], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true });
  t.after(() => child.kill());
  const replies = new Map(), notifications = [], protocolErrors = [];
  let buffer = '', next = 0;
  child.stdout.setEncoding('utf8');
  child.stdout.on('data', chunk => {
    buffer += chunk;
    let newline;
    while ((newline = buffer.indexOf('\n')) >= 0) {
      const message = JSON.parse(buffer.slice(0, newline)); buffer = buffer.slice(newline + 1);
      if (message.id === null && message.error) protocolErrors.push(message.error);
      else if (message.method) notifications.push(message);
      else { const resolve = replies.get(message.id); replies.delete(message.id); resolve?.(message); }
    }
  });
  const call = (method, params = {}) => new Promise(resolve => {
    const id = ++next;
    replies.set(id, resolve);
    child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
  });
  const file = hostStatusFile(root, process.pid);
  const data = { ...sample({ state: 'building', started_at: new Date().toISOString() }), owner_pid: process.pid,
    publisher_pid: process.pid, heartbeat_at: new Date().toISOString() };
  await fs.writeFile(file, JSON.stringify(data));
  const initialized = await call('initialize', { protocol: 4, config: { status_dir: root } });
  assert.deepEqual(initialized.result.provides, ['ui-views']);
  assert.deepEqual(initialized.result.tools, []);
  assert.equal(initialized.result.views[0].status_bar, undefined);
  const enabled = await call('initialize', { protocol: 4, config: { status_dir: root, status_bar: true } });
  assert.equal(enabled.result.views[0].status_bar, true);
  child.stdin.write('null\n');
  assert.equal(values((await call('views/render', { view: 'status' })).result).Status, 'Building');
  assert.equal((await call('views/render', { view: 'status' })).result.nodes.at(-1).state, 'busy');
  assert.equal(protocolErrors[0].code, -32600);
  while (!notifications.length) await new Promise(resolve => setTimeout(resolve, 30));
  const before = notifications.length;
  await fs.writeFile(file, JSON.stringify({ ...data, state: 'ready', stage: 'Watching for changes', node_count: 10 }));
  while (notifications.length === before) await new Promise(resolve => setTimeout(resolve, 30));
  assert.equal(values((await call('views/render', { view: 'status' })).result).Status, 'Ready');
  assert.equal(notifications.at(-1).method, 'ui/changed');
  const exited = new Promise(resolve => child.once('exit', resolve));
  assert.ok((await call('shutdown')).result);
  assert.equal(await exited, 0);
});
