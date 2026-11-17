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
        p.x = Math.max(140, Math.min(860, p.x + Math.max(-12, Math.min(12, p.dx))));
        p.y = Math.max(140, Math.min(510, p.y + Math.max(-12, Math.min(12, p.dy))));
      }
    }
    return { points, links, groups };
  }

  const radius = point => Math.min(11, 4 + Math.sqrt(point.degree));
  // One bounded pass groups parallel and reciprocal edges. Canonical lanes keep
  // reverse edges on distinct curves without changing endpoint order.
  function routes(links) {
    const pairs = new Map();
    for (const edge of links) {
      const key = JSON.stringify([edge.source, edge.target].sort());
      if (!pairs.has(key)) pairs.set(key, []);
      pairs.get(key).push(edge);
    }
    const paths = new Map();
    for (const edges of pairs.values()) edges.forEach((edge, index) => {
      const a = edge.a, b = edge.b;
      const distance = Math.hypot(b.x - a.x, b.y - a.y);
      if (edge.source === edge.target || distance < radius(a) + radius(b) + 12) {
        const spread = 24 + 36 * (index + 1) / edges.length, r = radius(a) + 4;
        paths.set(edge, { d: `M ${a.x-r} ${a.y} C ${a.x-spread*2} ${a.y-spread*2}, ${b.x+spread*2} ${b.y-spread*2}, ${b.x+radius(b)+4} ${b.y}`,
          x: (a.x+b.x)/2, y: Math.min(a.y,b.y)-spread*1.5 });
        return;
      }
      const sign = edge.source < edge.target ? 1 : -1;
      const lane = (index - (edges.length - 1) / 2) * Math.min(32, 220 / edges.length);
      const cx = (a.x+b.x)/2 - (b.y-a.y)/distance*lane*sign;
      const cy = (a.y+b.y)/2 + (b.x-a.x)/distance*lane*sign;
      const startLength = Math.hypot(cx-a.x, cy-a.y), endLength = Math.hypot(cx-b.x, cy-b.y);
      const sx = a.x+(cx-a.x)/startLength*(radius(a)+4), sy = a.y+(cy-a.y)/startLength*(radius(a)+4);
      const ex = b.x+(cx-b.x)/endLength*(radius(b)+4), ey = b.y+(cy-b.y)/endLength*(radius(b)+4);
      paths.set(edge, { d: `M ${sx} ${sy} Q ${cx} ${cy} ${ex} ${ey}`, x: (sx+2*cx+ex)/4, y: (sy+2*cy+ey)/4 });
    });
    return paths;
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
    const markerId = `graph-${crypto.randomUUID()}`;
    const defs = svgElement('defs');
    for (const [kind, fill] of [['directed', '#c4d0e7'], ['unverified', '#10131e']]) {
      const marker = svgElement('marker', { id: `${markerId}-${kind}`, viewBox: '0 0 10 10', refX: 10, refY: 5,
        markerWidth: 9, markerHeight: 9, markerUnits: 'userSpaceOnUse', orient: 'auto' });
      marker.append(svgElement('path', { d: 'M 1 1 L 9 5 L 1 9 Z', fill, stroke: '#c4d0e7', 'stroke-width': 1.2 })); defs.append(marker);
    }
    svg.append(defs);
    const aside = element('div', 'graph-controls');
    const { points, links, groups } = positions((data.vertices || []).slice(0, 250), (data.edges || []).slice(0, 750));
    const visibleGroups = new Set(groups), vertexElements = new Map(), edgeElements = [];
    const colors = new Map(groups.map((group, i) => [group, palette[i % palette.length]]));
    let zoom = 1, panX = 0, panY = 0, drag, selected;
    const history = [];
    let direction = 'all', relation = 'all';
    const original = new Map(points.map(p => [p.id, { x: p.x, y: p.y }]));
    const pinned = new Map();
    const kind = edge => edge.metadata?.relation || 'Unspecified';
    const symmetric = edge => edge.metadata?.direction === 'symmetric';
    const allowed = edge => (relation === 'all' || kind(edge) === relation)
      && visibleGroups.has(edge.a.group) && visibleGroups.has(edge.b.group)
      && (!selected || direction === 'all' || (!symmetric(edge) && (direction === 'incoming' ? edge.target === selected : edge.source === selected)));
    const complete = edge => edge.metadata?.sentence || `${edge.a.label} -> ${edge.b.label}: ${edge.label} (meaning unverified; stored order only)`;
    const redraw = () => {
      const paths = routes(links);
      for (const [edge, group] of edgeElements) {
        const route = paths.get(edge);
        for (const path of group.querySelectorAll('path')) path.setAttribute('d', route.d);
        const text = group.querySelector('text'); text.setAttribute('x', route.x); text.setAttribute('y', route.y - 7);
      }
      for (const p of points) vertexElements.get(p.id)?.setAttribute('transform', `translate(${p.x} ${p.y})`);
    };
    const focusLayout = () => {
      for (const p of points) Object.assign(p, original.get(p.id));
      if (selected) {
        const incoming = new Set(), outgoing = new Set();
        for (const edge of links.filter(allowed)) {
          if (edge.metadata?.direction !== 'directed') continue;
          if (edge.target === selected && edge.source !== selected) incoming.add(edge.source);
          if (edge.source === selected && edge.target !== selected) outgoing.add(edge.target);
        }
        const both = [...incoming].filter(id => outgoing.has(id));
        const columns = [[260,[...incoming].filter(id=>!outgoing.has(id))], [740,[...outgoing].filter(id=>!incoming.has(id))], [500,both]];
        for (const [x, ids] of columns) ids.forEach((id,i) => { const p=points.find(p=>p.id===id); p.x=x; p.y=140+(i+1)*(x===500?130:370)/(ids.length+1); });
        const center=points.find(p=>p.id===selected); if(center) { center.x=500; center.y=325; }
      }
      for (const p of points) if (pinned.has(p.id)) Object.assign(p, pinned.get(p.id));
      redraw();
    };
    button('Back selection', () => { const id=history.pop(); if(id) select(points.find(p=>p.id===id), false); });
    button('Reset selection', () => { selected=undefined; history.length=0; direction='all'; relation='all'; directionSelect.value='all'; relationSelect.value='all'; focusLayout(); update(); detail.textContent='Select a node to inspect its source and relationships.'; });
    const transform = () => viewport.setAttribute('transform', `translate(${panX} ${panY}) scale(${zoom})`);
    const changeZoom = factor => {
      const next = Math.max(0.3, Math.min(6, zoom * factor));
      panX = 500 - (500 - panX) * next / zoom; panY = 325 - (325 - panY) * next / zoom;
      zoom = next; transform();
    };
    button('Zoom in', () => changeZoom(1.25));
    button('Zoom out', () => changeZoom(0.8));
    button('Reset view', () => { zoom = 1; panX = 0; panY = 0; transform(); });
    button('Reset positions', () => { pinned.clear(); focusLayout(); });
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
    const select = (point, remember = true) => {
      if (!point) return;
      if (remember && selected && selected !== point.id) { history.push(selected); if(history.length>32) history.shift(); }
      selected = point.id;
      focusLayout(); update();
      list.value = point.id;
    };
    const showDetails = () => {
      const point=points.find(p=>p.id===selected); if(!point) return;
      detail.replaceChildren(element('strong', '', point.label), element('p', '', point.group), element('pre', '', point.detail));
      const related=links.filter(e=>e.source===selected || e.target===selected);
      detail.append(element('p','',`Relationships found in rendered subset: ${related.filter(allowed).length} of ${related.length} after filters. Full-artifact relationships may be omitted by projection limits. Unverified directions describe stored order only.`));
      for (const [name, matches] of [
        ['Incoming',e=>!symmetric(e)&&e.target===selected], ['Outgoing',e=>!symmetric(e)&&e.source===selected],
        ['Symmetric',e=>symmetric(e)]]) {
        const all=related.filter(matches), edges=all.filter(allowed);
        detail.append(element('h4','',`${name}: ${edges.length} rendered relationships after filters (${all.length} before filters)`));
        if(!edges.length) detail.append(element('p','','None found in this rendered subset after filters.'));
        const types=[...new Set(edges.map(kind))].sort();
        let remaining=30;
        for(const type of types) {
          const items=edges.filter(e=>kind(e)===type), ul=element('ul');
          if(remaining>0) detail.append(element('strong','',type));
          for(const edge of items.slice(0,remaining)) {
            const other=edge.source===selected?edge.b:edge.a, li=element('li');
            li.append(element('p','',`${other.label}: ${complete(edge)}`),
              element('p','',`Raw: ${kind(edge)}; provenance: ${edge.metadata?.provenance || 'unavailable'}`),
              element('pre','',`Other endpoint definition: ${other.detail}`), element('pre','',edge.metadata?.detail || 'Edge evidence location unavailable'));
            ul.append(li);
          }
          if(ul.childNodes.length) detail.append(ul);
          remaining=Math.max(0,remaining-items.length);
        }
        if(edges.length>30) detail.append(element('p','',`${edges.length-30} additional relationships omitted from this list (30-item cap); narrow the relation filter or inspect an edge directly.`));
      }
      if (data.select_action) {
        const more = element('button', 'ghost', 'Explore connections'); more.type = 'button';
        more.addEventListener('click', () => expand(data.select_action, { node: point.id }, more)); detail.append(more);
      }
    };
    const directionSelect=element('select'), relationSelect=element('select');
    for(const value of ['all','incoming','outgoing']) { const option=element('option','',value); option.value=value; directionSelect.append(option); }
    for(const value of ['all', ...new Set(links.map(kind))]) { const option=element('option','',value); option.value=value; relationSelect.append(option); }
    const directionLabel=element('label','','Direction (relative to selected node)'), relationLabel=element('label','','Relation type');
    directionLabel.append(directionSelect); relationLabel.append(relationSelect);
    directionSelect.addEventListener('change',()=>{direction=directionSelect.value;focusLayout();update();});
    relationSelect.addEventListener('change',()=>{relation=relationSelect.value;focusLayout();update();});
    const update = () => {
      const neighbors=new Set([selected]);
      for(const edge of links.filter(allowed)) if(edge.source===selected || edge.target===selected) { neighbors.add(edge.source); neighbors.add(edge.target); }
      for (const p of points) {
        const node=vertexElements.get(p.id);
        node.classList.toggle('graph-hidden', !visibleGroups.has(p.group));
        node.classList.toggle('graph-selected', p.id===selected);
        node.classList.toggle('graph-dim', Boolean(selected)&&!neighbors.has(p.id));
      }
      for (const [edge, line] of edgeElements) {
        const connected=edge.source===selected || edge.target===selected;
        line.classList.toggle('graph-hidden', !allowed(edge));
        line.classList.toggle('graph-connected', Boolean(selected)&&connected);
        line.classList.toggle('graph-dim', Boolean(selected)&&!connected);
      }
      showDetails();
      const q = search.value.toLocaleLowerCase();
      const matches = points.filter(p => visibleGroups.has(p.group) && `${p.label} ${p.id} ${p.detail}`.toLocaleLowerCase().includes(q));
      list.replaceChildren(...matches.map(p => { const option = element('option', '', p.label); option.value = p.id; return option; }));
      if (selected && matches.some(p => p.id === selected)) list.value = selected;
      if (!matches.length) { const option = element('option', '', 'No matching visible nodes'); option.disabled = true; list.append(option); }
    };
    const paths = routes(links);
    const sentence = edge => edge.metadata?.sentence || `${edge.a.label} → ${edge.b.label}: ${edge.label} (meaning unverified; stored order only)`;
    const inspectEdge = edge => {
      for (const [other, node] of edgeElements) node.classList.toggle('graph-edge-selected', other === edge);
      detail.replaceChildren(element('strong', '', sentence(edge)),
        element('p', '', `Raw relation: ${edge.metadata?.relation || 'unavailable'}; provenance: ${edge.metadata?.provenance || 'unavailable'}`),
        element('pre', '', `Source: ${edge.source}\n${edge.a.detail}\nTarget: ${edge.target}\n${edge.b.detail}`),
        element('pre', '', edge.metadata?.detail || 'Edge evidence location unavailable'));
    };
    for (const edge of links) {
      const route = paths.get(edge), direction = edge.metadata?.direction || 'unverified';
      const line = svgElement('g', { tabindex: 0, role: 'button', 'aria-label': sentence(edge) });
      line.classList.add('graph-edge');
      const path = svgElement('path', { d: route.d, fill: 'none', stroke: colors.get(edge.a.group) });
      if (direction !== 'symmetric') path.setAttribute('marker-end', `url(#${markerId}-${direction})`);
      const hit = svgElement('path', { d: route.d, fill: 'none', stroke: 'transparent', 'stroke-width': 14 }); hit.classList.add('graph-edge-hit');
      const title = svgElement('title'); title.textContent = sentence(edge);
      const label = svgElement('text', { x: route.x, y: route.y-7, 'text-anchor': 'middle' }); label.textContent = edge.label;
      label.classList.add('graph-edge-label');
      if (edge.metadata?.provenance === 'INFERRED') path.setAttribute('stroke-dasharray', '5 4');
      line.append(hit, path, title, label);
      line.addEventListener('pointerdown', event => event.stopPropagation());
      line.addEventListener('click', () => inspectEdge(edge));
      line.addEventListener('keydown', event => { if (['Enter', ' '].includes(event.key)) { event.preventDefault(); inspectEdge(edge); } });
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
      let moving, suppressClick = false;
      const localPointer = event => {
        const matrix = viewport.getScreenCTM();
        return matrix ? new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse()) : null;
      };
      g.addEventListener('pointerdown', event => {
        event.stopPropagation();
        if (event.button !== 0 || moving) return;
        const local = localPointer(event);
        if (!local) return;
        suppressClick = false;
        moving = { id: event.pointerId, x: event.clientX, y: event.clientY,
          offsetX: point.x - local.x, offsetY: point.y - local.y, moved: false };
        g.setPointerCapture(event.pointerId);
      });
      g.addEventListener('pointermove', event => {
        if (!moving || moving.id !== event.pointerId) return;
        event.stopPropagation();
        if (!moving.moved && Math.hypot(event.clientX - moving.x, event.clientY - moving.y) < 4) return;
        const local = localPointer(event);
        if (!local) return;
        moving.moved = true;
        g.classList.add('graph-dragging');
        // Keep space for the bounded self-loop and arrow geometry.
        point.x = Math.max(140, Math.min(860, local.x + moving.offsetX));
        point.y = Math.max(140, Math.min(510, local.y + moving.offsetY));
        pinned.set(point.id, { x: point.x, y: point.y });
        redraw();
      });
      const finishMove = event => {
        if (!moving || moving.id !== event.pointerId) return;
        suppressClick = moving.moved;
        moving = undefined;
        g.classList.remove('graph-dragging');
        if (g.hasPointerCapture(event.pointerId)) g.releasePointerCapture(event.pointerId);
      };
      g.addEventListener('pointerup', finishMove);
      g.addEventListener('pointercancel', finishMove);
      g.addEventListener('lostpointercapture', finishMove);
      g.addEventListener('click', event => {
        if (suppressClick && event.detail !== 0) { suppressClick = false; return; }
        select(point);
      });
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
    aside.append(searchLabel, list, directionLabel, relationLabel, legend, element('p', 'graph-direction-legend',
      'Filled arrows: relationship direction. Hollow arrows: stored order, meaning unverified. No arrow: symmetric. Colors: source groups. EXTRACTED: source-derived; INFERRED: inferred, dashed. Neither proves correctness. Details cover rendered edges only.'), detail);
    if (!points.length) stage.append(element('p', 'graph-empty', 'No nodes in this view. Try a different workspace search.'));
    body.append(stage, aside); box.append(toolbar, body); update();
    return box;
  }
  return { render, routes };
})();
