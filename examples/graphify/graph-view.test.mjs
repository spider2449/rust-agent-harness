import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { GraphView, projectGraph, describeEdge, relations, levelGraph, graphLevels } from './graph-view.mjs';

test('readable semantics retain raw provenance, unknown orientation and symmetric identity', () => {
  const names = new Map([['a', { label: 'A' }], ['b', { label: 'B' }]]);
  for (const [relation, verb] of Object.entries(relations)) {
    const edge = describeEdge({ source: 'a', target: 'b', relation, confidence: 'INFERRED' }, names);
    assert.equal(edge.metadata.sentence, `A ${verb} B`);
    assert.equal(edge.metadata.direction, 'directed');
    assert.equal(edge.metadata.relation, relation);
    assert.equal(edge.metadata.provenance, 'INFERRED');
    assert.equal(edge.source, 'a'); assert.equal(edge.target, 'b');
  }
  assert.match(describeEdge({ source: 'a', target: 'b', relation: 'uses' }, names).metadata.sentence, /meaning unverified/);
  assert.equal(describeEdge({ source: 'a', target: 'b', relation: 'semantically_similar_to' }, names).metadata.direction, 'symmetric');
  assert.ok(projectGraph(graph()).node.edges.every(e => !Object.hasOwn(e, 'metadata')));
  assert.equal(projectGraph(graph(), { metadata: true }).node.edges[0].metadata.direction, 'directed');
});

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

test('search ranks exact symbols, paths and filenames before truncating partial matches', () => {
  const nodes = Array.from({ length: 170 }, (_, i) => ({ id: `parser.rs:${i}` }));
  nodes.push(
    { id: 'nested', source_file: 'src/utils/parser.rs' },
    { id: 'root', source_file: 'parser.rs' },
    { id: 'symbol', label: 'Parser.rs' },
    { id: 'parser.rs' },
  );
  const data = { nodes, edges: [] };
  const ranked = projectGraph(data, { query: 'PARSER.RS' });
  assert.equal(ranked.matches, 174);
  assert.equal(ranked.node.vertices.length, 160);
  assert.deepEqual(ranked.node.vertices.slice(0, 5).map(n => n.id),
    ['symbol', 'parser.rs', 'root', 'nested', 'parser.rs:0']);
  assert.equal(projectGraph(data, { query: 'undefined' }).matches, 0);
  for (const source of ['src/utils/parser.rs', 'src\\utils\\parser.rs']) {
    nodes[170].source_file = source;
    for (const query of ['SRC/UTILS/PARSER.RS', 'src\\utils\\parser.rs']) {
      assert.deepEqual(projectGraph(data, { query }).node.vertices.map(n => n.id), ['nested']);
    }
  }
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
  const explained=projectGraph(long,{query:'Node',metadata:true});
  assert.ok(Buffer.byteLength(JSON.stringify(explained.node))<=140*1024);
  const explainedIds=new Set(explained.node.vertices.map(n=>n.id));
  assert.ok(explained.node.edges.every(e=>explainedIds.has(e.source)&&explainedIds.has(e.target)));
});

test('full-artifact direction and relation filters preserve order before caps', () => {
  const data = graph();
  data.edges.push({ source: 'other', target: 'entry', relation: 'imports' });
  const incoming = projectGraph(data, { focus: 'entry', direction: 'incoming', metadata: true });
  assert.deepEqual(incoming.node.edges.map(e => [e.source, e.target]), [['other', 'entry']]);
  const calls = projectGraph(data, { focus: 'entry', relation: 'calls', metadata: true });
  assert.equal(calls.node.edges.length, 1);
  assert.equal(calls.node.edges[0].metadata.sentence, '<entry> calls Other');
  assert.equal(calls.totalEdges, 3);
  assert.equal(calls.filteredEdges, 1);
});

test('levels require evidence; file aggregation preserves relation, provenance and direction', () => {
  const data=graph();
  data.nodes[2]._callable=true; data.nodes[3]._callable=true;
  data.edges.push({...data.edges[1]});
  assert.deepEqual(graphLevels(data), ['artifact','packages','files','symbols']);
  assert.equal(levelGraph(data).level,'packages');
  assert.deepEqual(levelGraph(data,'symbols').graph.nodes.map(n=>n.id),['entry','other']);
  const files=levelGraph(data,'files');
  const calls=files.graph.edges.find(e=>e.relation==='calls');
  assert.equal(calls.source,'file:plugins/a/lib.rs');
  assert.equal(calls.target,'file:crates/b/lib.rs');
  assert.equal(calls.aggregation_count,2);
  const projection=projectGraph(files.graph,{query:'lib.rs',metadata:true});
  assert.match(projection.node.edges.find(e=>e.metadata.relation==='calls').metadata.sentence,/Aggregated calls.*2 underlying/);
  assert.match(projection.node.edges[0].metadata.detail,/individual evidence locations unavailable/);
  assert.equal(levelGraph({nodes:[{id:'a'}],edges:[]}).level,'artifact');
  assert.throws(()=>levelGraph({nodes:[],edges:[]},'symbols'),/Unsupported/);
  const noPackages={nodes:data.nodes.slice(2),edges:data.edges.slice(1)};
  assert.equal(levelGraph(noPackages).level,'files');
});

test('cached graph views isolate sessions and workspace switches, reject invalid actions and stale artifacts', async t => {
  const root = await fs.realpath(await fs.mkdtemp(path.join(os.tmpdir(), 'rah-graph-view-')));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const workspaces = [path.join(root, 'one'), path.join(root, 'two')];
  for (const workspace of workspaces) {
    await fs.mkdir(path.join(workspace, '.rah/graphify-out'), { recursive: true });
    await fs.writeFile(path.join(workspace, '.rah/graphify-out/graph.json'), JSON.stringify(graph()));
  }
  const view = new GraphView({metadata:true}), data = { workspace: workspaces[0], state: 'ready' };
  const revision=(await view.render(data,'first')).find(n=>n.type==='graph').revision;
  assert.notEqual((await view.render({...data,workspace:workspaces[1]},'first')).find(n=>n.type==='graph').revision,revision);
  await view.act(data, 'first', 'graph_search', { query: 'entry', group: 'All groups' });
  assert.equal((await view.render(data, 'first')).find(n => n.type === 'graph').vertices[0].id, 'entry');
  assert.equal((await view.render(data, 'second')).find(n => n.type === 'graph').vertices[0].id, 'crate:a');
  assert.equal((await view.render({ ...data, workspace: workspaces[1] }, 'first')).find(n => n.type === 'graph').vertices[0].id, 'crate:a');
  await view.act(data, 'first', 'graph_focus', { node: 'other' });
  assert.equal((await view.render(data, 'first')).find(n => n.type === 'graph').vertices[0].id, 'other');
  await view.act(data, 'first', 'graph_back', {});
  assert.equal(view.selection('first', workspaces[0]).focus, '');
  for (let i=0; i<40; i++) await view.act(data, 'first', 'graph_focus', {node:'other'});
  assert.equal(view.selection('first', workspaces[0]).history.length, 32);
  await view.act(data,'first','graph_level',{level:'files'});
  assert.match((await view.render(data,'first')).map(n=>n.text||'').join(' '),/Aggregated file relations/);
  await view.act(data,'first','graph_focus',{node:'file:plugins/a/lib.rs'});
  assert.equal(view.selection('first',workspaces[0]).query,'plugins/a/lib.rs');
  assert.equal((await view.render(data,'first')).find(n=>n.type==='graph').vertices[0].id,'entry');
  await view.act(data,'first','graph_reset',{});
  assert.equal(view.selection('first',workspaces[0]).level,'auto');
  await assert.rejects(view.act(data, 'first', 'graph_filter', {direction:'sideways',relation:'calls'}), /Unknown/);
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
  assert.equal(view.selection('first',workspaces[0]).history.length,0);
  await fs.writeFile(path.join(data.workspace, '.rah/graphify-out/graph.json'), 'broken graph');
  assert.match((await view.render(data, 'first'))[1].text, /unavailable/);
  assert.equal(view.cache, null);
});
