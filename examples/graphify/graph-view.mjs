// Bounded projections of a validated workspace graph for declarative UI views.
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { readArtifact } from './artifact.mjs';

const clipped = (value, size) => [...String(value ?? '')].slice(0, size).join('');
const groupOf = node => {
  const parts = String(node.source_file || '').replaceAll('\\', '/').split('/');
  return clipped(['plugins', 'crates'].includes(parts[0]) ? parts.slice(0, 2).join('/') : parts[0] || 'External', 128);
};

// Endpoint conventions verified against graphifyy 0.9.107; see RELATIONS.md.
export const relations = Object.freeze({
  calls: 'calls', crate_depends_on: 'depends on', depends_on: 'depends on',
  imports: 'imports', contains: 'contains', method: 'has method',
  inherits: 'inherits from', implements: 'implements',
});
export function describeEdge(edge, byId) {
  const raw = clipped(edge.relation || 'related', 128);
  const provenance = clipped(edge.confidence || 'UNSPECIFIED', 128);
  const symmetric = raw === 'semantically_similar_to';
  const direction = symmetric ? 'symmetric' : Object.hasOwn(relations, raw) ? 'directed' : 'unverified';
  const label = symmetric ? `${raw} (symmetric)` : relations[raw] || `${raw} (meaning unverified)`;
  const name = id => clipped(byId.get(id)?.label || id, 128);
  return { source: edge.source, target: edge.target, label: clipped(`${label} · ${provenance}`, 128),
    metadata: { relation: raw, provenance, direction,
      sentence: clipped(edge.aggregation_count ? `Aggregated ${raw}: ${name(edge.source)} -> ${name(edge.target)}; ${edge.aggregation_count} underlying relationships (${direction})` : direction === 'symmetric'
        ? `${name(edge.source)} and ${name(edge.target)}: ${raw} (symmetric)`
        : `${name(edge.source)} ${label} ${name(edge.target)}${direction === 'unverified' ? '; stored source → target only' : ''}`, 512),
      detail: clipped(edge.aggregation_count ? `Aggregated ${edge.aggregation_count} artifact relationships; individual evidence locations unavailable in this summary` : edge.source_file ? `Edge evidence: ${edge.source_file} ${edge.source_location || '(line unavailable)'}` : 'Edge evidence location unavailable', 512),
    } };
}

export function graphLevels(graph) {
  const edges=graph.edges ?? graph.links;
  const packageIds=new Set(graph.nodes.filter(n=>n.id.startsWith('crate:') || n.type==='package').map(n=>n.id));
  return ['artifact', ...(edges.some(e=>packageIds.has(e.source)&&packageIds.has(e.target)&&['crate_depends_on','depends_on'].includes(e.relation))?['packages']:[]),
    ...(graph.nodes.some(n=>n.source_file)?['files']:[]), ...(graph.nodes.some(n=>n._callable===true)?['symbols']:[])];
}
export function levelGraph(graph, requested = 'auto') {
  const levels=graphLevels(graph);
  const level=requested==='auto' ? levels.includes('packages')?'packages':levels.includes('files')?'files':'artifact' : requested;
  if(!levels.includes(level)) throw new Error(`Unsupported graph level: ${level}`);
  const edges=graph.edges ?? graph.links;
  if(level==='artifact') return { graph, level, note:'Artifact nodes (mixed levels, unchanged relationships).' };
  if(level==='files') {
    const byId=new Map(graph.nodes.map(n=>[n.id,n])), files=new Map(), grouped=new Map();
    for(const n of graph.nodes) if(n.source_file) {
      const file=String(n.source_file).replaceAll('\\','/');
      if(!files.has(file)) files.set(file,{id:`file:${file}`,label:file,source_file:file,summary:true});
    }
    let omitted=0;
    for(const e of edges) {
      const a=byId.get(e.source)?.source_file?.replaceAll('\\','/'), b=byId.get(e.target)?.source_file?.replaceAll('\\','/');
      if(!a||!b) { omitted++; continue; }
      const key=JSON.stringify([a,b,e.relation||'related',e.confidence||'UNSPECIFIED']);
      if(!grouped.has(key)) grouped.set(key,{source:`file:${a}`,target:`file:${b}`,relation:e.relation,confidence:e.confidence,aggregation_count:0});
      grouped.get(key).aggregation_count++;
    }
    return {graph:{nodes:[...files.values()],edges:[...grouped.values()]},level,
      note:`Aggregated file relations: group by source file, target file, raw relation and provenance; retain direction and same-file loops. ${omitted} edges and ${graph.nodes.filter(n=>!n.source_file).length} nodes omitted because file endpoints are unavailable. Counts on edges describe underlying artifact relationships, not call sites.`};
  }
  const nodes=graph.nodes.filter(n=>level==='packages' ? n.id.startsWith('crate:')||n.type==='package' : n._callable===true);
  const ids=new Set(nodes.map(n=>n.id));
  const selected=edges.filter(e=>ids.has(e.source)&&ids.has(e.target)&&(level!=='packages'||['crate_depends_on','depends_on'].includes(e.relation)));
  return {graph:{nodes,edges:selected},level,note:level==='packages' ? 'Package dependencies emitted by the extractor; other relation types excluded.' : 'Callable symbol definitions marked by the extractor; other nodes excluded. This is not a complete symbol inventory.'};
}

export function projectGraph(graph, { query = '', group = 'All groups', focus = '', metadata = false, direction = 'all', relation = 'all' } = {}) {
  const all = graph.nodes, artifactEdges = graph.edges ?? graph.links;
  const edges = artifactEdges.filter(edge => (relation === 'all' || String(edge.relation || 'related') === relation)
    && (!focus || direction === 'all' || (edge.relation !== 'semantically_similar_to' && (direction === 'incoming' ? edge.target === focus : edge.source === focus))));
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
    detail: clipped(`${node.summary ? 'File summary' : 'Definition'}: ${node.source_file || (node.external_reference ? `External reference: ${node.external_reference}` : 'No source reference')} ${node.source_location || ''}\n${node.id}`, 512),
  }));
  const kept = new Set(vertices.map(node => node.id));
  const node = { type: 'graph', title: focus ? 'Selected node and direct relationships' : q ? `Search: ${clipped(query, 100)}` : 'Workspace relationships',
      vertices, edges: selectedEdges.filter(edge => kept.has(edge.source) && kept.has(edge.target)).slice(0, 750).map(edge => {
        const described = describeEdge(edge, byId);
        if (!metadata) delete described.metadata;
        return described;
      }), select_action: 'graph_focus' };
  // Leave room for the status, forms and summary within the host's 256 KiB view.
  while (Buffer.byteLength(JSON.stringify(node)) > 140 * 1024) {
    if (node.edges.length) node.edges.splice(Math.floor(node.edges.length * 0.8));
    else node.vertices.splice(Math.floor(node.vertices.length * 0.8));
  }
  return { groups: groups.slice(0, 100), matches, totalNodes: all.length, totalEdges: artifactEdges.length, filteredEdges: edges.length, queryEdges: selectedEdges.length, node };
}

export class GraphView {
  constructor({ metadata = false } = {}) { this.cache = null; this.selections = new Map(); this.metadata = metadata; }

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
      selection = { workspace, query: '', group: 'All groups', focus: '', direction: 'all', relation: 'all', history: [], snapshot: '', generation: 0, level: 'auto' };
      this.selections.set(key, selection);
      if (this.selections.size > 32) this.selections.delete(this.selections.keys().next().value);
    }
    return selection;
  }

  reconcile(selection, graph, key) {
    if (selection.snapshot === key) return;
    selection.history = [];
    selection.snapshot = key;
    if (selection.focus && !graph.nodes.some(node => node.id === selection.focus)) selection.focus = '';
    if (selection.level !== 'auto' && !graphLevels(graph).includes(selection.level)) selection.level = 'auto';
    if (selection.relation !== 'all' && !(graph.edges ?? graph.links).some(e => String(e.relation || 'related') === selection.relation)) selection.relation = 'all';
    if (selection.group !== 'All groups' && !graph.nodes.some(n => groupOf(n) === selection.group)) selection.group = 'All groups';
  }

  async render(data, session) {
    if (!data?.workspace) return [{ type: 'text', text: 'The relationship graph will appear after the first successful build.' }];
    try {
      const { graph, root, key } = await this.load(data.workspace);
      const selection = this.selection(session, root);
      this.reconcile(selection, graph, key);
      const current=levelGraph(graph, selection.query ? 'artifact' : selection.level);
      const projection = projectGraph(current.graph, { ...selection, metadata: this.metadata });
      if (this.metadata) projection.node.revision = createHash('sha256').update(`${key}:${session || ''}:${selection.generation}`).digest('hex');
      const stale = data.state !== 'ready';
      const generated = graph.graph?.rah?.generated_at || data.last_success_at || 'unknown';
      return [
        { type: 'heading', text: 'Relationship graph' },
        { type: 'text', text: `${stale ? 'Last available snapshot; indexing is not ready. ' : ''}Showing ${projection.node.vertices.length} / ${projection.totalNodes} nodes and ${projection.node.edges.length} / ${projection.totalEdges} edges. Generated: ${generated}. Artifact totals: ${graph.nodes.length} nodes, ${(graph.edges ?? graph.links).length} edges. Active level: ${current.level}${selection.query ? ' (full-artifact search overrides level)' : ''}; group: ${selection.group}; direction: ${selection.direction}; relation: ${selection.relation}. Search covers the full artifact; display is bounded to 250 nodes and 750 edges.` },
        { type: 'text', text: `${current.note} ${selection.level==='auto' && current.level!=='packages' ? 'Package dependencies unavailable; fallback shown.' : ''} Module-only view unavailable: no verified module hierarchy. Unsupported levels are not offered.` },
        { type: 'form', action: 'graph_level', submit: 'Change graph level', fields: [
          {name:'level',label:'Level',kind:'choice',value:selection.level,options:['auto',...graphLevels(graph)]},
        ] },
        { type: 'form', action: 'graph_search', submit: 'Search workspace graph', fields: [
          { name: 'query', label: 'Symbol, file or keyword', value: selection.query },
          { name: 'group', label: 'Source group', kind: 'choice', value: selection.group, options: ['All groups', ...projection.groups] },
        ] },
        { type: 'text', text: selection.focus ? `Focused node: ${clipped(selection.focus, 512)}`
          : selection.query ? `${projection.matches} matching nodes; the graph includes a bounded set of their direct neighbors.`
            : 'Architecture overview where crate metadata is available; otherwise the most connected nodes. Select a node to inspect or explore connections.' },
        { type: 'form', action: 'graph_filter', submit: 'Filter full-artifact relationships', fields: [
          { name: 'direction', label: 'Direction relative to expanded node', kind: 'choice', value: selection.direction, options: ['all','incoming','outgoing'] },
          { name: 'relation', label: 'Raw relation type', kind: 'choice', value: selection.relation, options: ['all', ...new Set((graph.edges ?? graph.links).map(e=>String(e.relation || 'related')).filter(r=>[...r].length<=128))].slice(0,101) },
        ] },
        { type: 'button', action: 'graph_back', label: 'Back expansion' },
        { type: 'button', action: 'graph_reset', label: 'Reset graph query' },
        { type: 'text', text: `Active-level edges after relationship filters: ${projection.filteredEdges}; edges between candidate nodes before display caps: ${projection.queryEdges}; matching seed nodes: ${projection.matches}. Display caps: 80 high-degree overview seeds when no crate overview exists, 160 search seeds, 250 total nodes, 750 edges, 140 KiB graph bytes; 100 source-group and relation choices (relation names over 128 characters unavailable as choices). Node IDs over 512 characters cannot be rendered. Node details cover rendered edges only, with 30 items per direction. Use full-artifact search, filters and explicit expansion to narrow omitted relationships.` },
        projection.node,
        { type: 'text', text: 'Experimental graph reference: missing relationships never prove absence of use. Known gaps: member reads/writes, property classification, Qt signal/slot wiring and instance-attribute calls. Use source search and inspection. Colors represent source groups. Arrows in node details follow stored source → target order. EXTRACTED and INFERRED are Graphify classifications; verify original source before relying on a relationship.' },
      ];
    } catch (error) {
      return [{ type: 'heading', text: 'Relationship graph' },
        { type: 'text', text: `Graph unavailable: ${clipped(error.message, 500)}. Waiting for a valid workspace artifact.` }];
    }
  }

  async act(data, session, action, input) {
    if (!data?.workspace) throw new Error('No active workspace graph');
    const { graph, root, key } = await this.load(data.workspace);
    const selection = this.selection(session, root);
    this.reconcile(selection, graph, key);
    const previous = { query: selection.query, group: selection.group, focus: selection.focus, direction: selection.direction, relation: selection.relation, level: selection.level };
    if (action === 'graph_reset') {
      Object.assign(selection, { query: '', group: 'All groups', focus: '', direction: 'all', relation: 'all', history: [], level: 'auto' });
    } else if (action === 'graph_back') {
      const previous = selection.history.pop(); if (previous) Object.assign(selection, previous);
    } else if (action === 'graph_level') {
      if(!['auto',...graphLevels(graph)].includes(input?.level)) throw new Error('Unsupported graph level');
      Object.assign(selection,{level:input.level,focus:'',query:'',group:'All groups'});
    } else if (action === 'graph_filter') {
      if (!['all','incoming','outgoing'].includes(input?.direction) || !['all', ...(graph.edges ?? graph.links).map(e=>String(e.relation || 'related'))].includes(input?.relation)) throw new Error('Unknown relationship filter');
      Object.assign(selection, { direction: input.direction, relation: input.relation });
    } else if (action === 'graph_search') {
      if (!input || typeof input.query !== 'string' || input.query.length > 256 || typeof input.group !== 'string') {
        throw new Error('Use a query of at most 256 characters and a source group');
      }
      const groups = new Set(['All groups', ...graph.nodes.map(groupOf)]);
      if (!groups.has(input.group)) throw new Error('Unknown source group');
      Object.assign(selection, { query: input.query.trim(), group: input.group, focus: '' });
    } else if (action === 'graph_focus') {
      const active=levelGraph(graph,selection.query?'artifact':selection.level).graph;
      if (!input || typeof input.node !== 'string' || ![...active.nodes, ...graph.nodes].some(node => node.id === input.node)) {
        throw new Error('Selected node is no longer present in this workspace');
      }
      if(input.node.startsWith('file:') && active.nodes.find(n=>n.id===input.node)?.summary) {
        Object.assign(selection,{query:input.node.slice(5),group:'All groups',focus:'',level:'artifact'});
      } else Object.assign(selection, { query: '', group: 'All groups', focus: input.node, level:selection.query || !active.nodes.some(n=>n.id===input.node)?'artifact':selection.level });
    } else throw new Error('Unknown graph action');
    if (!['graph_back','graph_reset'].includes(action)) {
      selection.history.push(previous); if(selection.history.length>32) selection.history.shift();
    }
    selection.generation++;
    return { rerender: true };
  }
}
