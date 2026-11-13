import assert from 'node:assert/strict';
import * as fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { readArtifact, queryArguments, describeTools, queryEvidence, responseStream, MAX_GRAPH_BYTES } from './artifact.mjs';
import { ManagedIndex, prepareGraph } from './managed-mcp.mjs';

async function fixture(t) {
  const root = await fs.realpath(await fs.mkdtemp(path.join(os.tmpdir(), 'rah-graph-artifact-')));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  await fs.writeFile(path.join(root, 'lib.rs'), 'pub fn entry() {}');
  const file = path.join(root, 'graph.json');
  const graph = { graph: { schema_version: 1, graphify_version: 'test' },
    nodes: [{ id: 'entry', source_file: 'lib.rs', source_location: 'L1' }, { id: 'external' }],
    edges: [{ source: 'entry', target: 'external', confidence: 'INFERRED' }] };
  return { root, file, graph, write: async data => fs.writeFile(file, JSON.stringify(data ?? graph)) };
}

test('valid artifacts retain confidence and report missing and unreferenced source honestly', async t => {
  const f = await fixture(t);
  f.graph.nodes.push({ id: 'missing', source_file: 'deleted.rs' });
  await f.write();
  const { graph, validation } = await readArtifact(f.file, f.root);
  assert.equal(graph.edges[0].confidence, 'INFERRED');
  assert.deepEqual(validation, { schema: 1, nodes: 3, edges: 1, source_files: 2,
    missing_source_files: 1, external_references: 0, nodes_without_source: 1 });
  delete f.graph.graph;
  f.graph.links = f.graph.edges;
  delete f.graph.edges;
  await f.write();
  assert.equal((await readArtifact(f.file, f.root)).validation.schema, 'legacy-unversioned');
});

test('Node built-in module specifiers are explicit external references, not filesystem paths', async t => {
  const f = await fixture(t);
  f.graph.nodes.push({ id: 'builtin', source_file: 'node:fs/promises' });
  await f.write();
  const { graph, validation } = await readArtifact(f.file, f.root);
  assert.equal(validation.external_references, 1);
  assert.equal(validation.source_files, 1);
  assert.equal(graph.nodes.at(-1).source_file, undefined);
  assert.equal(graph.nodes.at(-1).external_reference, 'node:fs/promises');
  await f.write(graph);
  assert.equal((await readArtifact(f.file, f.root)).validation.external_references, 1);
  f.graph.nodes.at(-1).source_file = 'javascript:alert(1)';
  await f.write();
  await assert.rejects(readArtifact(f.file, f.root), /escapes/);
});

test('invalid graphs, unknown versions, duplicate IDs and dangling edges fail closed', async t => {
  const f = await fixture(t);
  for (const graph of [null, {}, { nodes: [], edges: [], graph: { schema_version: 2 } },
    { nodes: [], edges: [], graph: { rah_placeholder: true } },
    { nodes: [{ id: 'a' }, { id: 'a' }], edges: [] },
    { nodes: [{ id: 1 }], edges: [] }, { nodes: [{ id: 'a' }], edges: [{ source: 'a', target: 'b' }] }]) {
    await fs.writeFile(f.file, JSON.stringify(graph));
    await assert.rejects(readArtifact(f.file, f.root));
  }
  const handle = await fs.open(f.file, 'w');
  await handle.truncate(MAX_GRAPH_BYTES + 1);
  await handle.close();
  await assert.rejects(readArtifact(f.file, f.root), /64 MiB/);
});

test('absolute, parent and symlink source escapes are rejected', async t => {
  const f = await fixture(t);
  for (const source of ['../outside.rs', 'nested\\..\\outside.rs', '/etc/passwd', 'C:\\secret.rs',
    '\\\\server\\share\\secret.rs', 'file:stream', 'bad\0file']) {
    f.graph.nodes[0].source_file = source;
    await f.write();
    await assert.rejects(readArtifact(f.file, f.root), /escapes/);
  }
  const outside = await fs.mkdtemp(path.join(os.tmpdir(), 'rah-graph-outside-'));
  t.after(() => fs.rm(outside, { recursive: true, force: true }));
  await fs.writeFile(path.join(outside, 'secret.rs'), 'secret');
  await fs.symlink(outside, path.join(f.root, 'linked'), process.platform === 'win32' ? 'junction' : 'dir');
  f.graph.nodes[0].source_file = 'linked/secret.rs';
  await f.write();
  await assert.rejects(readArtifact(f.file, f.root), /outside/);
});

test('publication records workspace evidence and preserves last good graph after validation failure', async t => {
  const f = await fixture(t);
  let signature = 'first', fail = false;
  const index = new ManagedIndex(f.root, async () => {
    await f.write(fail ? { nodes: [{ id: 'duplicate' }, { id: 'duplicate' }], edges: [] } : f.graph);
    return f.file;
  }, { scan: async () => signature });
  t.after(() => index.stop());
  await index.refresh();
  const published = path.join(f.root, '.rah/graphify-out/graph.json');
  const before = await fs.readFile(published, 'utf8');
  const graph = JSON.parse(before);
  assert.equal(graph.graph.rah.workspace, f.root);
  assert.equal(graph.graph.rah.fingerprint, 'first');
  assert.equal(graph.graph.rah.validation.schema, 1);
  assert.equal(graph.graph.rah.source_verification_required, true);
  fail = true; signature = 'second';
  await assert.rejects(index.refresh(), /unique/);
  assert.equal(await fs.readFile(published, 'utf8'), before);
  assert.equal(index.signature, 'first');
  await fs.writeFile(published, JSON.stringify({ nodes: [], edges: [], graph: { schema_version: 99 } }));
  await prepareGraph(f.root);
  assert.equal(JSON.parse(await fs.readFile(published, 'utf8')).graph.rah_placeholder, true);
});

test('queries are bounded and cannot select another workspace or tool', () => {
  assert.deepEqual(queryArguments({ name: 'query_graph', arguments: { question: 'ToolInvoker', depth: 1 } }),
    { question: 'ToolInvoker', depth: 1 });
  for (const args of [{ question: 'x', project_path: '../other' }, { question: '' },
    { question: 'x', depth: -1 }, { question: 'x', token_budget: 1000000 },
    { question: 'x', context_filter: [1] }, { question: 'x', mode: 'exec' },
    { question: 'x', demote_docs: 'false' }]) {
    assert.throws(() => queryArguments({ name: 'query_graph', arguments: args }));
  }
  assert.throws(() => queryArguments({ name: 'ingest', arguments: { question: 'x' } }));
});

test('tool discovery and results carry usage guidance, provenance and bounded graph context', () => {
  const listed = describeTools({ result: { tools: [{ name: 'query_graph', inputSchema: {
    properties: { question: { type: 'string' }, project_path: { type: 'string' } } } }, { name: 'ingest' }] } });
  assert.equal(listed.result.tools.length, 1);
  assert.match(listed.result.tools[0].description, /Read cited workspace source/);
  assert.match(listed.result.tools[0].description, /use this tool first before broad source searches/);
  assert.match(listed.result.tools[0].description, /do not wait for the user to ask for Graphify/);
  assert.match(listed.result.tools[0].description, /Skip it for a known-file edit/);
  assert.match(listed.result.tools[0].description, /unavailable graph requires source search/);
  assert.equal(listed.result.tools[0].inputSchema.properties.project_path, undefined);
  assert.equal(listed.result.tools[0].inputSchema.additionalProperties, false);
  const result = queryEvidence({ result: { content: [{ type: 'text', text: '界'.repeat(50000) }] } },
    { workspace: 'workspace', fingerprint: 'abc' });
  const text = result.result.content[0].text;
  assert.match(text, /workspace.*fingerprint/);
  assert.match(text, /untrusted navigation/);
  assert.match(text, /truncated/);
  assert.ok(Buffer.byteLength(text) < 64 * 1024);
  const error = { result: { isError: true } };
  assert.equal(queryEvidence(error, {}), error);
  const stale = queryEvidence({ id: 7, result: { content: [{ type: 'text', text: 'stale source result' }] } }, {}, false);
  assert.equal(stale.id, 7);
  assert.equal(stale.result.isError, true);
  assert.match(stale.result.content[0].text, /changed during query/);
  assert.ok(!JSON.stringify(stale).includes('stale source result'));
});

test('MCP stream handles fragmented multibyte messages, asynchronous transforms and malformed output', async () => {
  const stream = responseStream(async message => ({ ...message, checked: true }));
  const results = [];
  const complete = (async () => { for await (const data of stream) results.push(data.toString()); })();
  stream.write('{"id":1,"text":"界');
  stream.end('"}\n{"id":2}\n');
  await complete;
  assert.deepEqual(results.join('').trim().split('\n').map(JSON.parse),
    [{ id: 1, text: '界', checked: true }, { id: 2, checked: true }]);
  for (const text of ['bad\n', '{"id":1}', 'x'.repeat(8 * 1024 * 1024 + 1)]) {
    const invalid = responseStream(value => value);
    const failed = assert.rejects(async () => { for await (const data of invalid) void data; });
    invalid.end(text);
    await failed;
  }
});
