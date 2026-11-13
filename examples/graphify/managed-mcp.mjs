// Graphify's MCP launcher. RAH's sandbox owns this process and its descendants.
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { StatusReporter } from './status.mjs';
import { readArtifact, queryArguments, describeTools, queryEvidence, responseStream } from './artifact.mjs';

const PACKAGE = 'graphifyy[mcp]==0.9.107';
const SKIP = new Set(['.git', '.rah', 'graphify-out', 'node_modules', 'target',
  'dist', 'build', '.venv', 'venv', '__pycache__', '.cache', '.next', '.turbo']);

async function terminate(child) {
  if (!child.pid || child.exitCode !== null) return;
  if (process.platform === 'win32') {
    // Do not detach: the outer RAH Job Object remains the lifetime owner.
    await new Promise(resolve => {
      const killer = spawn(path.join(process.env.SYSTEMROOT || 'C:\\Windows', 'System32', 'taskkill.exe'),
        ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, stdio: 'ignore' });
      const timer = setTimeout(() => { killer.kill(); child.kill(); resolve(); }, 3000);
      killer.once('error', () => { clearTimeout(timer); child.kill(); resolve(); });
      killer.once('exit', () => { clearTimeout(timer); resolve(); });
    });
  } else {
    child.kill('SIGTERM');
    // The RAH supervisor additionally terminates the complete process group.
  }
}

export function run(program, args, { cwd, signal, owned, onLine, env = process.env, timeout = 180000, limit = 4 * 1024 * 1024 } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(program, args, { cwd, env, windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'] });
    owned?.add(child);
    const output = [], errors = [];
    let size = 0, failure, lines = '';
    const stop = reason => {
      if (failure) return;
      failure = reason;
      void terminate(child);
    };
    const abort = () => stop(new Error('Graphify operation cancelled'));
    const timer = setTimeout(() => stop(new Error('Graphify subprocess timed out')), timeout);
    signal?.addEventListener('abort', abort, { once: true });
    if (signal?.aborted) abort();
    for (const [stream, parts] of [[child.stdout, output], [child.stderr, errors]]) {
      stream.on('data', chunk => {
        size += chunk.length;
        if (size > limit) stop(new Error('Graphify subprocess output limit exceeded'));
        else {
          parts.push(chunk);
          if (stream === child.stdout && onLine) {
            lines += chunk.toString('utf8');
            let newline;
            while ((newline = lines.indexOf('\n')) >= 0) {
              onLine(lines.slice(0, newline)); lines = lines.slice(newline + 1);
            }
            lines = lines.slice(-8192);
          }
        }
      });
    }
    child.once('error', error => { failure = error; });
    child.once('close', code => {
      owned?.delete(child);
      clearTimeout(timer);
      signal?.removeEventListener('abort', abort);
      const stdout = Buffer.concat(output).toString('utf8');
      const stderr = Buffer.concat(errors).toString('utf8');
      if (failure) reject(failure);
      else if (code !== 0) reject(new Error(`${program} exited ${code}: ${stderr.slice(-3000)}`));
      else resolve(stdout);
    });
  });
}

export async function fingerprint(root, signal, owned) {
  let files;
  try {
    const output = await run('git', ['-c', 'core.fsmonitor=false', '-c', 'core.untrackedCache=false',
      'ls-files', '--cached', '--others', '--exclude-standard', '-z'], { cwd: root, signal, owned, timeout: 10000 });
    files = output.split('\0').filter(Boolean);
  } catch (error) {
    if (signal?.aborted) throw error;
    if (error.code !== 'ENOENT' && !String(error.message).includes('not a git repository')) throw error;
    files = ['.'];
  }
  const found = new Map();
  async function visit(relative) {
    if (signal?.aborted) throw new Error('Graphify operation cancelled');
    const parts = relative.replaceAll('\\', '/').split('/').filter(p => p && p !== '.');
    if (parts.includes('..') || parts.some(p => SKIP.has(p)) || path.isAbsolute(relative)) return;
    const normalized = parts.join('/');
    const file = path.join(root, ...parts);
    let stat;
    try { stat = await fs.lstat(file, { bigint: true }); }
    catch (error) { if (error.code === 'ENOENT') return; throw error; }
    if (stat.isSymbolicLink()) return;
    if (stat.isDirectory()) {
      for (const name of (await fs.readdir(file)).sort()) await visit(path.join(normalized, name));
    } else if (stat.isFile()) {
      found.set(normalized, `${stat.size}:${stat.mtimeNs}:${stat.ctimeNs}`);
      if (found.size > 100000) throw new Error('Graphify workspace exceeds 100000 files');
    }
  }
  for (const file of files) await visit(file);
  const hash = createHash('sha256');
  for (const [file, stat] of [...found].sort(([a], [b]) => a.localeCompare(b))) hash.update(`${file}\0${stat}\0`);
  return hash.digest('hex');
}

async function directory(parent, name) {
  const target = path.join(parent, name);
  await fs.mkdir(target, { recursive: true });
  if ((await fs.lstat(target)).isSymbolicLink() || path.dirname(await fs.realpath(target)) !== parent) {
    throw new Error(`Graphify index directory escapes its owner: ${target}`);
  }
  return fs.realpath(target);
}

async function stamp(file) {
  try { const stat = await fs.stat(file, { bigint: true }); return `${stat.size}:${stat.mtimeNs}`; }
  catch (error) { if (error.code === 'ENOENT') return undefined; throw error; }
}

export async function prepareGraph(root) {
  const home = await directory(root, '.rah');
  const output = await directory(home, 'graphify-out');
  const file = path.join(output, 'graph.json');
  let prior;
  try {
    const { graph } = await readArtifact(file, root);
    const stat = await fs.stat(file);
    prior = { node_count: graph.nodes.length,
      edge_count: (graph.edges ?? graph.links).length, last_success_at: stat.mtime.toISOString() };
  } catch (error) {
    // A corrupt or unsupported prior graph must not prevent MCP initialization.
    if (error.code && error.code !== 'ENOENT') throw error;
  }
  if (!prior) {
    const temporary = path.join(output, `placeholder-${randomUUID()}.tmp`);
    try {
      await fs.writeFile(temporary, JSON.stringify({ nodes: [], edges: [], graph: { rah_placeholder: true } }));
      await fs.rename(temporary, file);
    } finally { await fs.unlink(temporary).catch(() => {}); }
  }
  return prior || {};
}

export class ManagedIndex {
  constructor(root, build, { scan = fingerprint, log = () => {}, report = async () => {} } = {}) {
    this.root = root;
    this.build = build;
    this.scan = scan;
    this.log = log;
    this.report = report;
    this.controller = new AbortController();
    this.signature = undefined;
    this.active = undefined;
    this.timer = undefined;
  }

  refresh() {
    if (this.controller.signal.aborted) return Promise.reject(new Error('Graphify index stopped'));
    if (!this.active) {
      this.active = this.update().catch(async error => {
        if (!this.controller.signal.aborted) await this.report({ state: 'error', stage: 'Graph update failed', error: error.message });
        throw error;
      }).finally(() => { this.active = undefined; });
    }
    return this.active;
  }

  async update() {
    const signal = this.controller.signal;
    const home = await directory(this.root, '.rah');
    const output = await directory(home, 'graphify-out');
    const staging = await directory(home, 'graphify-staging');
    for (;;) {
      const before = await this.scan(this.root, signal);
      if (before === this.signature && this.published === await stamp(path.join(output, 'graph.json'))) return;
      const stage = await fs.mkdtemp(path.join(staging, 'build-'));
      let temporary;
      try {
        const started = Date.now();
        await this.report({ state: 'building', stage: 'Scanning workspace', started_at: new Date(started).toISOString(), progress: null });
        this.log('building workspace graph');
        const generated = await this.build(stage, signal);
        if (signal.aborted) throw new Error('Graphify index stopped');
        const { graph, validation } = await readArtifact(generated, this.root);
        if (before !== await this.scan(this.root, signal)) continue;
        const evidence = { workspace: this.root, generated_at: new Date().toISOString(),
          fingerprint: before, fingerprint_kind: 'source-path-size-mtime-ctime-sha256',
          validation, source_verification_required: true };
        graph.graph = { ...graph.graph, rah: evidence };
        await this.report({ stage: 'Publishing graph' });
        temporary = path.join(output, `graph-${randomUUID()}.tmp`);
        await fs.writeFile(temporary, JSON.stringify(graph));
        if (signal.aborted) throw new Error('Graphify index stopped');
        await fs.rename(temporary, path.join(output, 'graph.json'));
        temporary = undefined;
        this.published = await stamp(path.join(output, 'graph.json'));
        this.signature = before;
        this.evidence = evidence;
        await this.report({ state: 'ready', stage: 'Watching for changes', node_count: graph.nodes.length,
          edge_count: (graph.edges ?? graph.links).length, last_success_at: new Date().toISOString(),
          duration_ms: Date.now() - started, error: null });
        this.log(`graph ready (${graph.nodes.length} nodes)`);
        return;
      } finally {
        if (temporary) await fs.unlink(temporary).catch(() => {});
        // Only remove a directory this invocation created inside its checked owner.
        if (path.dirname(stage) !== staging || !(path.basename(stage).startsWith('build-'))) {
          throw new Error('Refusing to remove an unowned Graphify staging directory');
        }
        await fs.rm(stage, { recursive: true, force: true });
      }
    }
  }

  watch(interval = 2500) {
    if (this.timer || this.controller.signal.aborted) return;
    const tick = async () => {
      this.timer = undefined;
      try { await this.refresh(); }
      catch (error) { if (!this.controller.signal.aborted) this.log(`update failed: ${error.message}`); }
      if (!this.controller.signal.aborted) this.timer = setTimeout(tick, interval);
    };
    this.timer = setTimeout(tick, interval);
  }

  async stop() {
    this.controller.abort();
    clearTimeout(this.timer);
    this.timer = undefined;
    await this.active?.catch(() => {});
  }
}

export async function serve({ root = process.cwd(), uvx = 'uvx', interval = 2500 } = {}) {
  root = await fs.realpath(root);
  const children = new Set();
  const log = message => process.stderr.write(`[graphify] ${message}\n`);
  const status = new StatusReporter(root, { directory: process.env.RAH_GRAPHIFY_STATUS_DIR, local: false, log });
  const prefix = ['--offline', '--from', PACKAGE];
  const index = new ManagedIndex(root, async (stage, signal) => {
    const args = [...prefix, 'python', fileURLToPath(new URL('./extract.py', import.meta.url)), 'extract', root, '--code-only', '--no-cluster',
      '--max-workers', '4', '--exclude', '.rah/', '--out', stage];
    if (await fs.stat(path.join(root, 'Cargo.toml')).then(s => s.isFile(), error => {
      if (error.code === 'ENOENT') return false; throw error;
    })) args.push('--cargo');
    await run(uvx, args, { cwd: root, signal, owned: children,
      env: { ...process.env, PYTHONHASHSEED: process.env.PYTHONHASHSEED ?? '0' },
      onLine: line => { void status.progress(line); } });
    return path.join(stage, 'graphify-out', 'graph.json');
  }, { log, report: fields => status.update(fields), scan: (workspace, signal) => fingerprint(workspace, signal, children) });
  let server, stopping = false;
  const pending = new Map();
  const forwarded = new Map();
  let pendingBytes = 0;
  async function stop() {
    if (stopping) return;
    stopping = true;
    for (const request of pending.values()) request.cancelled = true;
    await Promise.all([index.stop(), ...[...children].map(terminate)]);
    await status.stop();
    process.stdin.destroy();
  }
  const onSignal = () => { void stop().finally(() => process.exit(0)); };
  process.once('SIGTERM', onSignal);
  process.once('SIGINT', onSignal);
  process.stdin.once('end', onSignal);
  try {
    const prior = await prepareGraph(root);
    status.files.push(path.join(root, '.rah/graphify-out/status.json'));
    await status.update({ ...prior, state: 'waiting', stage: 'Starting automatic indexing' });
    status.startHeartbeat();
    if (stopping) return;
    server = spawn(uvx, [...prefix, 'graphify-mcp', '--graph', '.rah/graphify-out/graph.json'],
      { cwd: root, env: process.env, windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    children.add(server);
    server.stdout.setEncoding('utf8');
    const responses = responseStream(async message => {
      const request = forwarded.get(message.id);
      if (!request) return message;
      forwarded.delete(message.id);
      if (request.method === 'tools/list') return describeTools(message);
      if (request.method === 'tools/call' && !message.error && !message.result?.isError) {
        const fresh = request.signature === await index.scan(root, index.controller.signal)
          && request.published === await stamp(path.join(root, '.rah/graphify-out/graph.json'));
        return queryEvidence(message, request.evidence, fresh);
      }
      return message;
    });
    responses.on('error', error => { log(error.message); onSignal(); });
    server.stdout.pipe(responses).pipe(process.stdout, { end: false });
    server.stderr.pipe(process.stderr);
    index.watch(interval);
    // Initialization stays responsive; model requests below still await refresh.
    void index.refresh().catch(error => log(error.message));
    const finished = new Promise((resolve, reject) => {
      server.once('error', reject);
      server.once('exit', code => { children.delete(server); stopping ? resolve() : reject(new Error(`Graphify MCP exited ${code}`)); });
    });
    const send = raw => {
      if (!stopping && !server.stdin.destroyed && !server.stdin.write(raw + '\n')) process.stdin.pause();
    };
    server.stdin.on('drain', () => { if (!stopping) process.stdin.resume(); });
    server.stdin.on('error', error => { log(error.message); onSignal(); });
    const failure = (id, message) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id,
      result: { isError: true, content: [{ type: 'text', text: `Graphify index unavailable: ${message}` }] } }) + '\n');
    let buffer = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', chunk => {
      buffer += chunk;
      if (Buffer.byteLength(buffer) > 8 * 1024 * 1024) { log('MCP input limit exceeded'); onSignal(); return; }
      let newline;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const raw = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
        if (!raw.trim()) continue;
        let message;
        try { message = JSON.parse(raw); } catch { log('Invalid MCP JSON'); onSignal(); return; }
        if (message.method === 'notifications/cancelled') {
          const request = pending.get(message.params?.requestId);
          if (request) request.cancelled = true;
        }
        if (message.method !== 'tools/call') {
          if (message.id !== undefined) {
            if (forwarded.size >= 32 || forwarded.has(message.id) || pending.has(message.id)) {
              failure(message.id, 'too many outstanding requests or duplicate id'); continue;
            }
            forwarded.set(message.id, { method: message.method });
          }
          send(raw); continue;
        }
        try { queryArguments(message.params); }
        catch (error) { failure(message.id, error.message); continue; }
        if (pending.size + forwarded.size >= 32 || pendingBytes + Buffer.byteLength(raw) > 4 * 1024 * 1024
          || pending.has(message.id) || forwarded.has(message.id)) {
          failure(message.id, 'too many queued requests or duplicate id'); continue;
        }
        const request = { cancelled: false };
        pendingBytes += Buffer.byteLength(raw);
        pending.set(message.id, request);
        void index.refresh().then(() => {
          if (!request.cancelled) {
            forwarded.set(message.id, { method: message.method, signature: index.signature,
              published: index.published, evidence: index.evidence });
            send(raw);
          }
        }, error => {
          if (!request.cancelled && !stopping) failure(message.id, error.message);
        }).finally(() => {
          pendingBytes -= Buffer.byteLength(raw);
          if (pending.get(message.id) === request) pending.delete(message.id);
        });
      }
    });
    process.stdin.resume();
    await finished;
  } catch (error) {
    await status.update({ state: 'error', stage: 'Indexer failed', error: error.message });
    throw error;
  } finally {
    await stop();
    process.removeListener('SIGTERM', onSignal);
    process.removeListener('SIGINT', onSignal);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  if (args.length && !(args[0] === '--uvx' && args.length === 2)) throw new Error('Usage: node managed-mcp.mjs [--uvx PATH]');
  const uvx = args.length ? args[1] : 'uvx';
  serve({ uvx }).catch(error => { process.stderr.write(`[graphify] ${error.message}\n`); process.exitCode = 1; });
}
