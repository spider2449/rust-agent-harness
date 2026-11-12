// A read-only RAH process-plugin view; no tools or host-service access.
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { hostStatusFile, readStatus } from './status.mjs';

const text = (value, max = 1500) => typeof value === 'string' ? value.slice(0, max) : '';
const count = value => Number.isSafeInteger(value) && value >= 0 ? value.toLocaleString('en-US') : '—';
const date = value => Number.isFinite(Date.parse(value)) ? new Date(value).toLocaleString(undefined, { hour12: false }) : '—';
const alive = pid => {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try { process.kill(pid, 0); return true; } catch (error) { return error.code === 'EPERM'; }
};

export function renderStatus(data, { now = Date.now(), owner = process.ppid, isAlive = alive, statusBar = false } = {}) {
  if (data && data.owner_pid !== owner) throw new Error('Status belongs to another RAH process');
  let state = data?.state || 'waiting';
  let stage = text(data?.stage) || 'Waiting for graph indexing to start';
  if (data && state !== 'stopped') {
    if (!isAlive(data.publisher_pid)) { state = 'stopped'; stage = 'Indexer is not running'; }
    else if (!Number.isFinite(Date.parse(data.heartbeat_at)) || now - Date.parse(data.heartbeat_at) > 15000) {
      state = 'unresponsive'; stage = 'Indexer has stopped reporting its status';
    }
  }
  const labels = { waiting: 'Waiting', building: 'Building', ready: 'Ready', error: 'Update failed', stopped: 'Stopped', unresponsive: 'Not responding' };
  const progress = data?.progress;
  const validProgress = Number.isSafeInteger(progress?.completed) && progress.completed >= 0
    && Number.isSafeInteger(progress?.total) && progress.total >= progress.completed;
  const duration = state === 'building' && Number.isFinite(Date.parse(data?.started_at))
    ? Math.max(0, now - Date.parse(data.started_at)) : data?.duration_ms;
  const items = [
    { key: 'Status', value: Object.hasOwn(labels, state) ? labels[state] : 'Unknown' },
    { key: 'Stage', value: stage },
    { key: 'Workspace', value: text(data?.workspace, 4096) || '—' },
    { key: 'Files', value: validProgress ? `${count(progress.completed)} / ${count(progress.total)}` : '—' },
    { key: 'Nodes', value: count(data?.node_count) },
    { key: 'Edges', value: count(data?.edge_count) },
    { key: 'Last indexed', value: date(data?.last_success_at) },
    { key: state === 'building' ? 'Elapsed' : 'Build duration', value: typeof duration === 'number' && Number.isFinite(duration) && duration >= 0 ? `${(duration / 1000).toFixed(1)} s` : '—' },
  ];
  if (data?.error) items.push({ key: 'Last error', value: text(data.error) });
  const nodes = [{ type: 'heading', text: 'Graphify' }, { type: 'key_value', items },
    { type: 'text', text: state === 'ready' ? 'Watching for changes. Updates are automatic.' : 'This page refreshes automatically.' }];
  if (statusBar) {
    const states = { waiting: 'idle', building: 'busy', ready: 'ready', error: 'error', stopped: 'idle', unresponsive: 'warning' };
    const label = state === 'building' && validProgress ? `Graphify ${progress.completed}/${progress.total}`
      : state === 'error' ? 'Graphify failed' : state === 'stopped' ? 'Graphify stopped' : 'Graphify';
    const detail = [`Graphify: ${Object.hasOwn(labels, state) ? labels[state] : 'Unknown'}`,
      ...items.filter(item => ['Stage', 'Files', 'Workspace', 'Last indexed', 'Last error'].includes(item.key)).map(item => `${item.key}: ${item.value}`)].join('\n');
    nodes.push({ type: 'status', icon: 'graph', label, state: Object.hasOwn(states, state) ? states[state] : 'warning', detail: [...detail].slice(0, 1024).join('') });
  }
  return { nodes };
}

export async function loadView(directory, options = {}) {
  try { return renderStatus(await readStatus(hostStatusFile(directory, options.owner ?? process.ppid)), options); }
  catch (error) {
    const nodes = [{ type: 'heading', text: 'Graphify' }, { type: 'text', text: `Status unavailable: ${text(error.message)}` }];
    if (options.statusBar) nodes.push({ type: 'status', icon: 'graph', label: 'Graphify', state: 'warning', detail: [...`Graphify: status unavailable\n${error.message}`].slice(0, 1024).join('') });
    return { nodes };
  }
}

export function serveStatusView() {
  let directory, timer, stopped = false, polling = false, previous, buffer = '', options = {};
  const send = message => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...message }) + '\n');
  const stop = () => { stopped = true; clearInterval(timer); };
  const poll = async () => {
    if (polling || stopped || !directory) return;
    polling = true;
    try {
      const current = JSON.stringify(await loadView(directory, options));
      if (!stopped && current !== previous) { previous = current; send({ method: 'ui/changed', params: { view: 'status' } }); }
    } finally { polling = false; }
  };
  async function request(message) {
    if (!message || typeof message !== 'object' || Array.isArray(message)) {
      send({ id: null, error: { code: -32600, message: 'Invalid request' } }); return;
    }
    if (message.id === undefined) return;
    try {
      let result;
      if (message.method === 'initialize') {
        const configured = message.params?.config?.status_dir;
        if (typeof configured !== 'string' || !path.isAbsolute(configured)) throw new Error('status_dir must be an absolute directory');
        directory = configured;
        const enabled = message.params?.config?.status_bar;
        if (enabled !== undefined && typeof enabled !== 'boolean') throw new Error('status_bar must be boolean');
        options = { statusBar: enabled === true };
        result = { protocol: 4, version: '0.1.0', provides: ['ui-views'], requires: [], tools: [], views: [{ id: 'status', title: 'Graphify' }] };
        if (options.statusBar) result.views[0].status_bar = true;
        if (!timer) timer = setInterval(() => { void poll(); }, 1000);
      } else if (message.method === 'views/render' && message.params?.view === 'status') {
        if (!directory) throw new Error('Plugin has not initialized');
        result = await loadView(directory, options);
      } else if (message.method === 'shutdown') {
        stop();
        process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: message.id, result: {} }) + '\n', () => process.exit(0));
        return;
      } else {
        send({ id: message.id, error: { code: -32601, message: 'Unknown method or view' } }); return;
      }
      send({ id: message.id, result });
    } catch (error) { send({ id: message.id, error: { code: -32602, message: text(error.message) } }); }
  }
  process.stdin.setEncoding('utf8');
  process.stdin.on('data', chunk => {
    buffer += chunk;
    if (Buffer.byteLength(buffer) > 1024 * 1024) { stop(); process.exit(1); }
    let newline;
    while ((newline = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
      if (!line.trim()) continue;
      try { void request(JSON.parse(line)); } catch { send({ id: null, error: { code: -32700, message: 'Invalid JSON' } }); }
    }
  });
  process.stdin.once('end', () => { stop(); process.exit(0); });
  process.once('SIGTERM', () => { stop(); process.exit(0); });
  process.once('SIGINT', () => { stop(); process.exit(0); });
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) serveStatusView();
