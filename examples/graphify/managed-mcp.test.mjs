import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { ManagedIndex, fingerprint, run, prepareGraph, backendCommands } from './managed-mcp.mjs';
import { StatusReporter, readStatus } from './status.mjs';

async function fixture(t, build) {
  const root = await fs.realpath(await fs.mkdtemp(path.join(os.tmpdir(), 'rah-graphify-')));
  t.after(async () => { await index.stop(); await fs.rm(root, { recursive: true, force: true }); });
  await fs.writeFile(path.join(root, 'lib.rs'), 'original');
  let builds = 0;
  const index = new ManagedIndex(root, async (stage, signal) => {
    builds++;
    const label = await fs.readFile(path.join(root, 'lib.rs'), 'utf8');
    if (build) await build({ root, stage, signal, builds });
    const file = path.join(stage, 'graph.json');
    await fs.writeFile(file, JSON.stringify({ nodes: [{ id: label }], edges: [] }));
    return file;
  });
  return { root, index, count: () => builds, graph: path.join(root, '.rah/graphify-out/graph.json'),
    label: async () => JSON.parse(await fs.readFile(path.join(root, '.rah/graphify-out/graph.json'), 'utf8')).nodes[0].id };
}

test('first use generates a graph; unchanged scans do not rebuild or self-trigger', async t => {
  const f = await fixture(t);
  await f.index.refresh();
  assert.equal(await f.label(), 'original');
  await f.index.refresh();
  assert.equal(f.count(), 1);
  assert.deepEqual(await fs.readdir(path.join(f.root, '.rah/graphify-staging')), []);
});

test('direct Python backend uses isolated module execution and never invokes uv', () => {
  const python = backendCommands({ python: 'portable/python.exe' });
  assert.equal(python.program, 'portable/python.exe');
  assert.equal(python.extract[0], '-I');
  assert.ok(python.extract[1].endsWith('extract.py'));
  assert.deepEqual(python.server, ['-I', '-m', 'graphify.serve']);
  const uvx = backendCommands({ uvx: 'custom/uvx.exe' });
  assert.equal(uvx.program, 'custom/uvx.exe');
  assert.ok(uvx.server.includes('graphify-mcp'));
});

test('source edits, additions, deletion and renames invalidate the graph', async t => {
  const f = await fixture(t);
  await f.index.refresh();
  await fs.writeFile(path.join(f.root, 'lib.rs'), 'changed-source');
  await f.index.refresh();
  assert.equal(await f.label(), 'changed-source');
  await fs.writeFile(path.join(f.root, 'extra.rs'), 'pub fn added() {}');
  await f.index.refresh();
  await fs.rename(path.join(f.root, 'extra.rs'), path.join(f.root, 'renamed.rs'));
  await f.index.refresh();
  await fs.unlink(path.join(f.root, 'renamed.rs'));
  await f.index.refresh();
  assert.equal(f.count(), 5);
});

test('concurrent freshness requests join one build', async t => {
  let begin, finish;
  const started = new Promise(resolve => { begin = resolve; });
  const release = new Promise(resolve => { finish = resolve; });
  const f = await fixture(t, async () => { begin(); await release; });
  const reporter = new StatusReporter(f.root);
  f.index.report = fields => reporter.update(fields);
  await prepareGraph(f.root);
  const first = f.index.refresh();
  await started;
  assert.equal((await readStatus(path.join(f.root, '.rah/graphify-out/status.json'))).state, 'building');
  assert.equal(JSON.parse(await fs.readFile(f.graph, 'utf8')).graph.rah_placeholder, true);
  const second = f.index.refresh();
  assert.equal(first, second);
  finish();
  await Promise.all([first, second]);
  assert.equal(f.count(), 1);
  assert.equal((await readStatus(path.join(f.root, '.rah/graphify-out/status.json'))).state, 'ready');
  assert.equal(JSON.parse(await fs.readFile(f.graph, 'utf8')).graph?.rah_placeholder, undefined);
});

test('edits during extraction discard the incomplete snapshot and rebuild', async t => {
  const f = await fixture(t, async ({ root, builds }) => {
    if (builds === 1) await fs.writeFile(path.join(root, 'lib.rs'), 'edited-during-extraction');
  });
  await f.index.refresh();
  assert.equal(f.count(), 2);
  assert.equal(await f.label(), 'edited-during-extraction');
});

test('failed updates preserve the last good graph and remain dirty for retry', async t => {
  let fail = false;
  const f = await fixture(t, async () => { if (fail) throw new Error('extract failed'); });
  await f.index.refresh();
  fail = true;
  await fs.writeFile(path.join(f.root, 'lib.rs'), 'new-source');
  await assert.rejects(f.index.refresh(), /extract failed/);
  assert.equal(await f.label(), 'original');
  fail = false;
  await f.index.refresh();
  assert.equal(await f.label(), 'new-source');
});

test('missing or externally corrupted derived graphs are regenerated', async t => {
  const f = await fixture(t);
  await f.index.refresh();
  await fs.unlink(f.graph);
  await f.index.refresh();
  await fs.writeFile(f.graph, 'broken');
  await f.index.refresh();
  assert.equal(await f.label(), 'original');
  assert.equal(f.count(), 3);
});

test('cancellation aborts a build, cleans staging and prevents publication', async t => {
  let begin;
  const started = new Promise(resolve => { begin = resolve; });
  const f = await fixture(t, async ({ signal }) => {
    begin();
    await new Promise((_, reject) => signal.addEventListener('abort', () => reject(new Error('cancelled')), { once: true }));
  });
  const building = assert.rejects(f.index.refresh(), /cancelled/);
  await started;
  await f.index.stop();
  await building;
  await assert.rejects(fs.stat(f.graph), { code: 'ENOENT' });
  await assert.rejects(f.index.refresh(), /stopped/);
  assert.deepEqual(await fs.readdir(path.join(f.root, '.rah/graphify-staging')), []);
});

test('the watcher refreshes changes without a query and stops with its owner', { timeout: 5000 }, async t => {
  const f = await fixture(t);
  await f.index.refresh();
  let updated;
  const ready = new Promise(resolve => { updated = resolve; });
  f.index.log = message => { if (message.startsWith('graph ready')) updated(); };
  f.index.watch(10);
  await fs.writeFile(path.join(f.root, 'lib.rs'), 'watcher-updated-source');
  await ready;
  assert.equal(await f.label(), 'watcher-updated-source');
  await f.index.stop();
  assert.equal(f.index.timer, undefined);
  assert.equal(f.index.controller.signal.aborted, true);
});

test('source fingerprint excludes dependency and index output', async t => {
  const f = await fixture(t);
  const before = await fingerprint(f.root);
  for (const name of ['node_modules', '.rah', 'target']) {
    await fs.mkdir(path.join(f.root, name), { recursive: true });
    await fs.writeFile(path.join(f.root, name, 'output.rs'), 'ignored');
  }
  assert.equal(await fingerprint(f.root), before);
});

test('subprocess output and time are bounded', { timeout: 10000 }, async () => {
  await assert.rejects(run(process.execPath, ['-e', "console.log('x'.repeat(10000))"], { limit: 32 }), /output limit/);
  await assert.rejects(run(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { timeout: 50 }), /timed out/);
});

test('extraction environment overrides stay in the owned child', async () => {
  const before = process.env.PYTHONHASHSEED;
  assert.equal(await run(process.execPath, ['-e', 'process.stdout.write(process.env.PYTHONHASHSEED)'],
    { env: { ...process.env, PYTHONHASHSEED: '0' } }), '0');
  assert.equal(process.env.PYTHONHASHSEED, before);
});

test('subprocess cancellation terminates the owned command', { timeout: 10000 }, async () => {
  const controller = new AbortController();
  const owned = new Set();
  const running = run(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { signal: controller.signal, owned });
  const rejected = assert.rejects(running, /cancelled/);
  controller.abort();
  await rejected;
  assert.equal(owned.size, 0);
});
