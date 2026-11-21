import assert from 'node:assert/strict';
import { once } from 'node:events';
import { PassThrough } from 'node:stream';
import test from 'node:test';
import { queryArguments, serve } from './cbm-readonly.mjs';

test('only bounded read-only arguments can reach the pinned project', () => {
  assert.throws(() => queryArguments('index_repository', {}), /unavailable/);
  assert.throws(() => queryArguments('search_graph', { query: 'symbol', project: 'other' }), /Unknown/);
  assert.throws(() => queryArguments('trace_path', { function_name: 'symbol', mode: 'run' }), /mode/);
  assert.throws(() => queryArguments('trace_path', { function_name: 'symbol', depth: 4 }), /depth/);
  assert.throws(() => queryArguments('get_code_snippet', { qualified_name: 'symbol', max_lines: 100 }), /max_lines/);
  assert.throws(() => queryArguments('query_graph', { query: 'x'.repeat(4097) }), /query/);
  const trace = queryArguments('trace_path', { function_name: 'symbol' });
  assert.equal(trace.project, 'rah-workspace');
  assert.equal(trace.mode, 'calls');
  assert.equal(trace.include_evidence, true);
});

test('MCP preserves errors and evidence, caps output and cancels active work on disconnect', async () => {
  const input = new PassThrough();
  const output = new PassThrough();
  let cancelled;
  const stopped = new Promise(resolve => { cancelled = resolve; });
  serve(input, output, async (name, args, signal) => {
    if (name === 'query_graph') return { content: [{ type: 'text', text: 'x'.repeat(50000) }] };
    if (name === 'get_code_snippet') throw new Error('CBM index is missing for the active workspace');
    if (name === 'trace_path') return { content: [], structuredContent: { strategy: 'unique_name', confidence: 0.38 } };
    await new Promise(resolve => signal.addEventListener('abort', resolve, { once: true }));
    cancelled();
    return { content: [] };
  });
  async function call(id, name, args) {
    const response = once(output, 'data');
    input.write(JSON.stringify({ id, method: 'tools/call', params: { name, arguments: args } }) + '\n');
    return JSON.parse((await response)[0]).result;
  }
  assert.match((await call(1, 'query_graph', { query: 'MATCH (n) RETURN n' })).content[0].text, /output limit/);
  assert.match((await call(2, 'get_code_snippet', { qualified_name: 'symbol' })).content[0].text, /index is missing/);
  assert.deepEqual((await call(3, 'trace_path', { function_name: 'symbol' })).structuredContent,
    { strategy: 'unique_name', confidence: 0.38 });
  input.write(JSON.stringify({ id: 4, method: 'tools/call', params: {
    name: 'search_graph', arguments: { query: 'symbol' },
  } }) + '\n');
  input.end();
  await stopped;
});
