// Bounded projections of a validated workspace graph for declarative UI views.
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { readArtifact } from './artifact.mjs';

const clipped = (value, size) => [...String(value ?? '')].slice(0, size).join('');
const groupOf = node => {
  const parts = String(node.source_file || '').replaceAll('\\', '/').split('/');
  return clipped(['plugins', 'crates'].includes(parts[0]) ? parts.slice(0, 2).join('/') : parts[0] || 'External', 128);
};

export function projectGraph(graph, { query = '', group = 'All groups', focus = '' } = {}) {
  const all = graph.nodes, edges = graph.edges ?? graph.links;
  const byId = new Map(all.map(node => [node.id, node]));
  const degree = new Map(all.map(node => [node.id, 0]));
  for (const edge of edges) {
    degree.set(edge.source, degree.get(edge.source) + 1);
    degree.set(edge.target, degree.get(edge.target) + 1);
  }
  const groups = [...new Set(all.map(groupOf))].sort();
  const inGroup = node => group === 'All groups' || groupOf(node) === group;
  const q = query.toLocaleLowerCase();
  let candidates = all.filter(inGroup);
  let seeds;
  if (focus && byId.has(focus)) seeds = [byId.get(focus)];
  else if (q) seeds = candidates.filter(node => `${node.id} ${node.label} ${node.source_file}`.toLocaleLowerCase().includes(q));
  else {
    // Prefer the architectural layer, then the most connected symbols.
    seeds = candidates.filter(node => node.id.startsWith('crate:'));
    if (!seeds.length) seeds = [...candidates].sort((a, b) => degree.get(b.id) - degree.get(a.id)).slice(0, 80);
  }
  const matches = seeds.length;
  const ids = new Set(seeds.slice(0, 160).map(node => node.id));
  if (q || focus || !seeds.some(node => node.id.startsWith('crate:'))) {
    const neighbors = new Set();
    for (const edge of edges) {
      if (ids.has(edge.source)) neighbors.add(edge.target);
      if (ids.has(edge.target)) neighbors.add(edge.source);
    }
    candidates = [...neighbors].map(id => byId.get(id)).filter(inGroup)
      .sort((a, b) => degree.get(b.id) - degree.get(a.id));
    for (const node of candidates) { if (ids.size >= 250) break; ids.add(node.id); }
  }
  const selectedEdges = edges.filter(edge => ids.has(edge.source) && ids.has(edge.target));
  const vertices = [...ids].map(id => byId.get(id)).filter(node => node.id.length <= 512).map(node => ({
    id: node.id, label: clipped(node.label || node.id, 128), group: groupOf(node),
    detail: clipped(`${node.source_file || (node.external_reference ? `External reference: ${node.external_reference}` : 'No source reference')} ${node.source_location || ''}\n${node.id}`, 512),
  }));
  const kept = new Set(vertices.map(node => node.id));
  const node = { type: 'graph', title: focus ? 'Selected node and direct relationships' : q ? `Search: ${clipped(query, 100)}` : 'Workspace relationships',
      vertices, edges: selectedEdges.filter(edge => kept.has(edge.source) && kept.has(edge.target)).slice(0, 750).map(edge => ({
        source: edge.source, target: edge.target,
        label: clipped(`${edge.relation || 'related'} · ${edge.confidence || 'UNSPECIFIED'}`, 128),
      })), select_action: 'graph_focus' };
  // Leave room for the status, forms and summary within the host's 256 KiB view.
  while (Buffer.byteLength(JSON.stringify(node)) > 180 * 1024) {
    if (node.edges.length) node.edges.splice(Math.floor(node.edges.length * 0.8));
    else node.vertices.splice(Math.floor(node.vertices.length * 0.8));
  }
  return { groups: groups.slice(0, 100), matches, totalNodes: all.length, totalEdges: edges.length, node };
}

export class GraphView {
  constructor() { this.cache = null; this.selections = new Map(); }

  async load(workspace) {
    const root = await fs.realpath(workspace);
    const file = path.join(root, '.rah/graphify-out/graph.json');
    const stat = await fs.lstat(file, { bigint: true });
    const key = `${root}:${stat.size}:${stat.mtimeNs}:${stat.ctimeNs}`;
    if (this.cache?.key !== key) {
      this.cache = null;
      const { graph, validation } = await readArtifact(file, root);
      this.cache = { key, root, graph, validation };
    }
    return this.cache;
  }

  selection(session, workspace) {
    const key = typeof session === 'string' ? session : '';
    let selection = this.selections.get(key);
    if (!selection || selection.workspace !== workspace) {
      selection = { workspace, query: '', group: 'All groups', focus: '' };
      this.selections.set(key, selection);
      if (this.selections.size > 32) this.selections.delete(this.selections.keys().next().value);
    }
    return selection;
  }

  async render(data, session) {
    if (!data?.workspace) return [{ type: 'text', text: 'The relationship graph will appear after the first successful build.' }];
    try {
      const { graph, root } = await this.load(data.workspace);
      const selection = this.selection(session, root);
      if (selection.focus && !graph.nodes.some(node => node.id === selection.focus)) selection.focus = '';
      const projection = projectGraph(graph, selection);
      const stale = data.state !== 'ready';
      const generated = graph.graph?.rah?.generated_at || data.last_success_at || 'unknown';
      return [
        { type: 'heading', text: 'Relationship graph' },
        { type: 'text', text: `${stale ? 'Last available snapshot; indexing is not ready. ' : ''}Showing ${projection.node.vertices.length} / ${projection.totalNodes} nodes and ${projection.node.edges.length} / ${projection.totalEdges} edges. Generated: ${generated}. Search covers the full artifact; display is bounded to 250 nodes and 750 edges.` },
        { type: 'form', action: 'graph_search', submit: 'Search workspace graph', fields: [
          { name: 'query', label: 'Symbol, file or keyword', value: selection.query },
          { name: 'group', label: 'Source group', kind: 'choice', value: selection.group, options: ['All groups', ...projection.groups] },
        ] },
        { type: 'text', text: selection.focus ? `Focused node: ${clipped(selection.focus, 512)}`
          : selection.query ? `${projection.matches} matching nodes; the graph includes a bounded set of their direct neighbors.`
            : 'Architecture overview where crate metadata is available; otherwise the most connected nodes. Select a node to inspect or explore connections.' },
        projection.node,
        { type: 'text', text: 'Colors represent source groups. Arrows in node details follow stored source → target order. EXTRACTED and INFERRED are Graphify classifications; verify original source before relying on a relationship.' },
      ];
    } catch (error) {
      return [{ type: 'heading', text: 'Relationship graph' },
        { type: 'text', text: `Graph unavailable: ${clipped(error.message, 500)}. Waiting for a valid workspace artifact.` }];
    }
  }

  async act(data, session, action, input) {
    if (!data?.workspace) throw new Error('No active workspace graph');
    const { graph, root } = await this.load(data.workspace);
    const selection = this.selection(session, root);
    if (action === 'graph_search') {
      if (!input || typeof input.query !== 'string' || input.query.length > 256 || typeof input.group !== 'string') {
        throw new Error('Use a query of at most 256 characters and a source group');
      }
      const groups = new Set(['All groups', ...graph.nodes.map(groupOf)]);
      if (!groups.has(input.group)) throw new Error('Unknown source group');
      Object.assign(selection, { query: input.query.trim(), group: input.group, focus: '' });
    } else if (action === 'graph_focus') {
      if (!input || typeof input.node !== 'string' || !graph.nodes.some(node => node.id === input.node)) {
        throw new Error('Selected node is no longer present in this workspace');
      }
      Object.assign(selection, { query: '', group: 'All groups', focus: input.node });
    } else throw new Error('Unknown graph action');
    return { rerender: true };
  }
}
