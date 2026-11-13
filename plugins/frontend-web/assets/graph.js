// Reusable, data-only relationship views. Provider strings are always text.
const RelationshipGraph = (() => {
  const palette = ['#65a9e8', '#f4a44b', '#df7185', '#68bfab', '#a994e8', '#d7c665', '#76bd78', '#d28fb8'];
  const element = (tag, cls, text) => {
    const node = document.createElement(tag);
    if (cls) node.className = cls;
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const svgElement = (tag, attrs = {}) => {
    const node = document.createElementNS('http://www.w3.org/2000/svg', tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, String(value));
    return node;
  };

  function positions(vertices, edges) {
    const groups = [...new Set(vertices.map(v => v.group))].sort();
    const points = vertices.map((v, i) => {
      const group = groups.indexOf(v.group), angle = group * Math.PI * 2 / Math.max(1, groups.length);
      return { ...v, x: 500 + Math.cos(angle) * 200 + Math.sin(i * 2.4) * 65,
        y: 325 + Math.sin(angle) * 170 + Math.cos(i * 2.4) * 65, dx: 0, dy: 0, degree: 0 };
    });
    const map = new Map(points.map(v => [v.id, v]));
    const links = edges.map(e => ({ ...e, a: map.get(e.source), b: map.get(e.target) })).filter(e => e.a && e.b);
    for (const e of links) { e.a.degree++; e.b.degree++; }
    // A fixed iteration budget gives deterministic layout without background tasks.
    for (let step = 0; step < 90; step++) {
      for (const p of points) { p.dx = (500 - p.x) * 0.006; p.dy = (325 - p.y) * 0.006; }
      for (let i = 0; i < points.length; i++) for (let j = i + 1; j < points.length; j++) {
        const a = points[i], b = points[j], dx = a.x - b.x || 0.1, dy = a.y - b.y || 0.1;
        const distance = Math.max(25, dx * dx + dy * dy), force = Math.min(2, 240 / distance);
        a.dx += dx * force; a.dy += dy * force; b.dx -= dx * force; b.dy -= dy * force;
      }
      for (const e of links) {
        const dx = e.b.x - e.a.x, dy = e.b.y - e.a.y;
        const force = (Math.hypot(dx, dy) - 65) * 0.0006;
        e.a.dx += dx * force; e.a.dy += dy * force; e.b.dx -= dx * force; e.b.dy -= dy * force;
      }
      for (const p of points) {
        p.x = Math.max(35, Math.min(965, p.x + Math.max(-12, Math.min(12, p.dx))));
        p.y = Math.max(35, Math.min(615, p.y + Math.max(-12, Math.min(12, p.dy))));
      }
    }
    return { points, links, groups };
  }

  function render(data, expand) {
    const box = element('section', 'relationship-graph');
    box.graphData = JSON.stringify(data);
    const toolbar = element('div', 'graph-toolbar');
    toolbar.append(element('strong', '', String(data.title || 'Relationships')));
    const button = (label, action) => {
      const b = element('button', 'ghost', label); b.type = 'button'; b.addEventListener('click', action);
      toolbar.append(b); return b;
    };
    const body = element('div', 'graph-body'), stage = element('div', 'graph-stage');
    const svg = svgElement('svg', { viewBox: '0 0 1000 650', role: 'group', 'aria-label': data.title || 'Relationship graph', tabindex: 0 });
    const viewport = svgElement('g'); svg.append(viewport); stage.append(svg);
    const aside = element('div', 'graph-controls');
    const { points, links, groups } = positions((data.vertices || []).slice(0, 250), (data.edges || []).slice(0, 750));
    const visibleGroups = new Set(groups), vertexElements = new Map(), edgeElements = [];
    const colors = new Map(groups.map((group, i) => [group, palette[i % palette.length]]));
    let zoom = 1, panX = 0, panY = 0, drag, selected;
    const transform = () => viewport.setAttribute('transform', `translate(${panX} ${panY}) scale(${zoom})`);
    const changeZoom = factor => {
      const next = Math.max(0.3, Math.min(6, zoom * factor));
      panX = 500 - (500 - panX) * next / zoom; panY = 325 - (325 - panY) * next / zoom;
      zoom = next; transform();
    };
    button('Zoom in', () => changeZoom(1.25));
    button('Zoom out', () => changeZoom(0.8));
    button('Reset view', () => { zoom = 1; panX = 0; panY = 0; transform(); });
    const maximize = button('Expand view', () => {
      const expanded = box.classList.toggle('graph-expanded');
      maximize.textContent = expanded ? 'Close expanded view' : 'Expand view';
      maximize.setAttribute('aria-expanded', String(expanded));
    });
    maximize.setAttribute('aria-expanded', 'false');
    box.addEventListener('keydown', event => {
      if (event.key === 'Escape' && box.classList.contains('graph-expanded')) { maximize.click(); maximize.focus(); }
    });
    svg.addEventListener('wheel', event => { event.preventDefault(); changeZoom(event.deltaY < 0 ? 1.1 : 1 / 1.1); }, { passive: false });
    svg.addEventListener('pointerdown', event => {
      if (event.button !== 0) return;
      const rect = svg.getBoundingClientRect();
      drag = { x: event.clientX, y: event.clientY, px: panX, py: panY, scale: Math.min(rect.width / 1000, rect.height / 650) };
      svg.setPointerCapture(event.pointerId);
    });
    svg.addEventListener('pointermove', event => {
      if (!drag) return;
      panX = drag.px + (event.clientX - drag.x) / Math.max(0.1, drag.scale);
      panY = drag.py + (event.clientY - drag.y) / Math.max(0.1, drag.scale); transform();
    });
    svg.addEventListener('pointerup', () => { drag = null; });
    svg.addEventListener('pointercancel', () => { drag = null; });
    const detail = element('div', 'graph-detail', 'Select a node to inspect its source and relationships.');
    detail.setAttribute('aria-live', 'polite');
    const searchLabel = element('label', '', 'Find visible nodes');
    const search = element('input'); search.type = 'search'; search.placeholder = 'Filter the node list'; searchLabel.append(search);
    const list = element('select', 'graph-node-list'); list.size = 6; list.setAttribute('aria-label', 'Graph nodes');
    const select = point => {
      selected = point.id;
      for (const [id, node] of vertexElements) node.classList.toggle('graph-selected', id === selected);
      for (const [edge, line] of edgeElements) line.classList.toggle('graph-connected', edge.source === selected || edge.target === selected);
      detail.replaceChildren(element('strong', '', point.label), element('p', '', point.group), element('pre', '', point.detail));
      const related = links.filter(e => e.source === selected || e.target === selected);
      const ul = element('ul');
      for (const edge of related.slice(0, 30)) ul.append(element('li', '',
        `${edge.source === selected ? '→' : '←'} ${edge.source === selected ? edge.b.label : edge.a.label}: ${edge.label}`));
      detail.append(ul);
      if (related.length > 30) detail.append(element('p', '', `${related.length - 30} more visible relationships`));
      if (data.select_action) {
        const more = element('button', 'ghost', 'Explore connections'); more.type = 'button';
        more.addEventListener('click', () => expand(data.select_action, { node: point.id }, more)); detail.append(more);
      }
      list.value = point.id;
    };
    const update = () => {
      for (const p of points) vertexElements.get(p.id).classList.toggle('graph-hidden', !visibleGroups.has(p.group));
      for (const [edge, line] of edgeElements) line.classList.toggle('graph-hidden', !visibleGroups.has(edge.a.group) || !visibleGroups.has(edge.b.group));
      const q = search.value.toLocaleLowerCase();
      const matches = points.filter(p => visibleGroups.has(p.group) && `${p.label} ${p.id} ${p.detail}`.toLocaleLowerCase().includes(q));
      list.replaceChildren(...matches.map(p => { const option = element('option', '', p.label); option.value = p.id; return option; }));
      if (selected && matches.some(p => p.id === selected)) list.value = selected;
      if (!matches.length) { const option = element('option', '', 'No matching visible nodes'); option.disabled = true; list.append(option); }
    };
    for (const edge of links) {
      const line = svgElement('line', { x1: edge.a.x, y1: edge.a.y, x2: edge.b.x, y2: edge.b.y, stroke: colors.get(edge.a.group) });
      const title = svgElement('title'); title.textContent = `${edge.a.label} → ${edge.b.label}: ${edge.label}`; line.append(title);
      viewport.append(line); edgeElements.push([edge, line]);
    }
    const labelBoxes = [];
    for (const point of points) {
      const g = svgElement('g', { transform: `translate(${point.x} ${point.y})`, tabindex: 0, role: 'button', 'aria-label': point.label });
      g.classList.add('graph-vertex'); g.dataset.nodeId = point.id;
      const circle = svgElement('circle', { r: Math.min(11, 4 + Math.sqrt(point.degree)), fill: colors.get(point.group) });
      const title = svgElement('title'); title.textContent = `${point.label}\n${point.detail}`;
      const label = svgElement('text', { x: 12, y: 4 }); label.textContent = point.label;
      if (point.degree < 8) label.classList.add('graph-minor-label');
      else {
        const width = Math.min(240, point.label.length * 6.5);
        for (const offset of [4, 22, -14, 40, -32, 58, -50, 76, -68]) {
          const candidate = { x: point.x + 12, y: point.y + offset - 11, width };
          if (labelBoxes.every(b => candidate.x > b.x + b.width || b.x > candidate.x + width
            || candidate.y > b.y + 15 || b.y > candidate.y + 15)) {
            label.setAttribute('y', offset); labelBoxes.push(candidate); break;
          }
        }
      }
      g.append(circle, title, label);
      g.addEventListener('pointerdown', event => event.stopPropagation());
      g.addEventListener('click', () => select(point));
      g.addEventListener('keydown', event => { if (['Enter', ' '].includes(event.key)) { event.preventDefault(); select(point); } });
      viewport.append(g); vertexElements.set(point.id, g);
    }
    const legend = element('fieldset', 'graph-legend'); legend.append(element('legend', '', 'Groups'));
    for (const group of groups) {
      const label = element('label'), checkbox = element('input'); checkbox.type = 'checkbox'; checkbox.checked = true;
      const swatch = svgElement('svg', { width: 12, height: 12, 'aria-hidden': true });
      swatch.append(svgElement('circle', { cx: 6, cy: 6, r: 5, fill: colors.get(group) }));
      label.append(checkbox, swatch, document.createTextNode(`${group || 'Other'} (${points.filter(p => p.group === group).length})`));
      checkbox.addEventListener('change', () => { checkbox.checked ? visibleGroups.add(group) : visibleGroups.delete(group); update(); });
      legend.append(label);
    }
    search.addEventListener('input', update);
    list.addEventListener('change', () => { const point = points.find(p => p.id === list.value); if (point) select(point); });
    aside.append(searchLabel, list, legend, detail);
    if (!points.length) stage.append(element('p', 'graph-empty', 'No nodes in this view. Try a different workspace search.'));
    body.append(stage, aside); box.append(toolbar, body); update();
    return box;
  }
  return { render };
})();
