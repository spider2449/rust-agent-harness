import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { GraphView, projectGraph } from './graph-view.mjs';

const graph = () => ({ nodes: [
  { id: 'crate:a', label: 'A', source_file: 'plugins/a/Cargo.toml' },
  { id: 'crate:b', label: 'B', source_file: 'crates/b/Cargo.toml' },
  { id: 'entry', label: '<entry>', source_file: 'plugins/a/lib.rs', source_location: 'L1' },
  { id: 'other', label: 'Other', source_file: 'crates/b/lib.rs' },
], edges: [
  { source: 'crate:a', target: 'crate:b', relation: 'depends_on', confidence: 'EXTRACTED' },
  { source: 'entry', target: 'other', relation: 'calls', confidence: 'INFERRED' },
] });

test('overview, full-artifact search, source groups and selected-node neighbors are distinct', () => {
  const data = graph();
  assert.deepEqual(projectGraph(data).node.vertices.map(n => n.id), ['crate:a', 'crate:b']);
  const search = projectGraph(data, { query: 'entry' });
  assert.deepEqual(search.node.vertices.map(n => n.id), ['entry', 'other']);
  assert.match(search.node.edges[0].label, /INFERRED/);
  assert.match(search.node.vertices[0].detail, /plugins\/a\/lib.rs L1/);
  assert.equal(projectGraph(data, { query: 'not found' }).node.vertices.length, 0);
  assert.equal(projectGraph(data, { query: 'entry', group: 'plugins/a' }).node.vertices.length, 1);
  assert.deepEqual(projectGraph(data, { focus: 'other' }).node.vertices.map(n => n.id), ['other', 'entry']);
});

test('large graph projections disclose totals and preserve valid endpoints within bounds', () => {
  const data = { nodes: Array.from({ length: 1200 }, (_, i) => ({ id: `node${i}`, label: `Node ${i}` })), edges: [] };
  for (let i = 1; i < 1200; i++) data.edges.push({ source: 'node0', target: `node${i}` });
  const result = projectGraph(data, { focus: 'node0' });
  assert.equal(result.totalNodes, 1200);
  assert.equal(result.node.vertices.length, 250);
  assert.ok(result.node.edges.length <= 750);
  const ids = new Set(result.node.vertices.map(n => n.id));
  assert.ok(result.node.edges.every(e => ids.has(e.source) && ids.has(e.target)));
  const long = { nodes: Array.from({ length: 300 }, (_, i) => ({ id: `${i}${'界'.repeat(200)}`,
    label: 'Node ' + '界'.repeat(120), source_file: '界'.repeat(150) })), edges: [] };
  for (let i = 0; i < 300; i++) for (let j = i + 1; j < Math.min(300, i + 10); j++) {
    long.edges.push({ source: long.nodes[i].id, target: long.nodes[j].id });
  }
  const bounded = projectGraph(long, { query: 'Node' });
  assert.ok(Buffer.byteLength(JSON.stringify(bounded.node)) <= 180 * 1024);
  const remaining = new Set(bounded.node.vertices.map(n => n.id));
  assert.ok(bounded.node.edges.every(e => remaining.has(e.source) && remaining.has(e.target)));
});

test('cached graph views isolate sessions and workspace switches, reject invalid actions and stale artifacts', async t => {
  const root = await fs.realpath(await fs.mkdtemp(path.join(os.tmpdir(), 'rah-graph-view-')));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const workspaces = [path.join(root, 'one'), path.join(root, 'two')];
  for (const workspace of workspaces) {
    await fs.mkdir(path.join(workspace, '.rah/graphify-out'), { recursive: true });
    await fs.writeFile(path.join(workspace, '.rah/graphify-out/graph.json'), JSON.stringify(graph()));
  }
  const view = new GraphView(), data = { workspace: workspaces[0], state: 'ready' };
  await view.act(data, 'first', 'graph_search', { query: 'entry', group: 'All groups' });
  assert.equal((await view.render(data, 'first')).find(n => n.type === 'graph').vertices[0].id, 'entry');
  assert.equal((await view.render(data, 'second')).find(n => n.type === 'graph').vertices[0].id, 'crate:a');
  assert.equal((await view.render({ ...data, workspace: workspaces[1] }, 'first')).find(n => n.type === 'graph').vertices[0].id, 'crate:a');
  await view.act(data, 'first', 'graph_focus', { node: 'other' });
  assert.equal((await view.render(data, 'first')).find(n => n.type === 'graph').vertices[0].id, 'other');
  assert.match((await view.render({ ...data, state: 'building' }, 'first'))[1].text, /not ready/);
  const cached = view.cache;
  await view.render(data, 'first');
  assert.equal(view.cache, cached);
  await assert.rejects(view.act(data, 'first', 'graph_focus', { node: 'missing' }), /no longer/);
  await assert.rejects(view.act(data, 'first', 'graph_search', { query: 'x'.repeat(257), group: 'All groups' }), /256/);
  await view.act(data, 'first', 'graph_focus', { node: 'other' });
  const changed = graph();
  changed.nodes = changed.nodes.filter(node => node.id !== 'other'); changed.edges = changed.edges.slice(0, 1);
  await fs.writeFile(path.join(data.workspace, '.rah/graphify-out/graph.json'), JSON.stringify(changed));
  assert.equal((await view.render(data, 'first')).find(node => node.type === 'graph').title, 'Workspace relationships');
  await fs.writeFile(path.join(data.workspace, '.rah/graphify-out/graph.json'), 'broken graph');
  assert.match((await view.render(data, 'first'))[1].text, /unavailable/);
  assert.equal(view.cache, null);
});
