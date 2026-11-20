import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';

const { routes, positions } = vm.runInNewContext(fs.readFileSync(new URL('../../plugins/frontend-web/assets/graph.js', import.meta.url), 'utf8') + '\nRelationshipGraph');

test('an isolated node stays in the initial overview beside a connected cluster', () => {
  const vertices = Array.from({ length: 100 }, (_, i) => ({ id: String(i), group: String(i % 4) }));
  const edges = vertices.slice(1, -1).map(v => ({ source: '0', target: v.id }));
  const { points } = positions(vertices, edges);
  const isolated = points.at(-1);
  assert.ok(Math.hypot(isolated.x - 500, isolated.y - 325) < 325);
});
test('bounded routes distinguish reciprocal, parallel, self and coincident edges', () => {
  const a = { x: 200, y: 300, degree: 3 }, b = { x: 400, y: 300, degree: 3 };
  const edges = [ { source: 'a', target: 'b', a, b }, { source: 'b', target: 'a', a: b, b: a },
    { source: 'a', target: 'b', a, b }, { source: 'a', target: 'a', a, b: a } ];
  const paths = [...routes(edges).values()];
  assert.equal(new Set(paths.map(p => p.d)).size, 4);
  assert.ok(paths[3].d.includes('C'));
  for (const route of paths) assert.doesNotMatch(route.d, /NaN|Infinity/);
  const path = routes([{ source: 'a', target: 'b', a, b: a }]).values().next().value;
  assert.ok(path.d.includes('C'));
  const single = routes([edges[0]]).get(edges[0]);
  const values = single.d.match(/-?\d+(?:\.\d+)?/g).map(Number);
  assert.ok(values[0] > a.x + 4);
  assert.ok(values.at(-2) < b.x - 4);
});
