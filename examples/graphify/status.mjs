// Bounded Graphify status artifacts shared by the launcher and its UI view.
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { randomUUID } from 'node:crypto';

export const MAX_STATUS_BYTES = 16384;
export const hostStatusFile = (directory, owner = process.ppid) => path.join(directory, `graphify-${owner}.json`);

export async function readStatus(file) {
  let handle;
  try {
    handle = await fs.open(file, 'r');
    const buffer = Buffer.alloc(MAX_STATUS_BYTES + 1);
    const { bytesRead } = await handle.read(buffer, 0, buffer.length, 0);
    if (bytesRead > MAX_STATUS_BYTES) throw new Error('Status document is too large');
    const value = JSON.parse(buffer.subarray(0, bytesRead).toString('utf8'));
    if (!value || value.version !== 1 || typeof value.state !== 'string' || typeof value.workspace !== 'string') {
      throw new Error('Unsupported status document');
    }
    return value;
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  } finally { await handle?.close(); }
}

export class StatusReporter {
  constructor(workspace, { directory, local = true, owner = process.ppid, publisher = process.pid, now = () => Date.now(), log = () => {} } = {}) {
    this.now = now;
    this.log = log;
    this.files = [];
    if (directory) this.files.push(hostStatusFile(directory, owner));
    if (local) this.files.push(path.join(workspace, '.rah/graphify-out/status.json'));
    this.value = { version: 1, workspace, owner_pid: owner, publisher_pid: publisher,
      state: 'waiting', stage: 'Waiting to start', progress: null, node_count: null, edge_count: null,
      started_at: null, last_success_at: null, duration_ms: null, error: null };
    this.queue = Promise.resolve();
  }

  update(fields = {}) {
    const time = new Date(this.now()).toISOString();
    this.value = { ...this.value, ...fields, updated_at: time, heartbeat_at: time };
    if (typeof this.value.error === 'string') this.value.error = this.value.error.slice(-1500);
    return this.write();
  }

  write() {
    const text = JSON.stringify(this.value);
    this.queue = this.queue.then(async () => {
      if (Buffer.byteLength(text) > MAX_STATUS_BYTES) throw new Error('Status document exceeds its limit');
      for (const file of this.files) {
        await fs.mkdir(path.dirname(file), { recursive: true });
        const temporary = `${file}.${randomUUID()}.tmp`;
        try { await fs.writeFile(temporary, text); await fs.rename(temporary, file); }
        finally { await fs.unlink(temporary).catch(() => {}); }
      }
    }).catch(error => this.log(`status unavailable: ${error.message}`));
    return this.queue;
  }

  startHeartbeat() {
    if (this.timer) return;
    this.timer = setInterval(() => {
      this.value = { ...this.value, heartbeat_at: new Date(this.now()).toISOString() };
      void this.write();
    }, 5000);
  }

  progress(line) {
    let match = line.match(/found (\d+) code/);
    if (match) return this.update({ stage: 'Parsing source files', progress: { completed: 0, total: Number(match[1]) } });
    match = line.match(/AST extraction: (\d+)\/(\d+) uncached files/);
    if (match) return this.update({ stage: Number(match[1]) === Number(match[2]) ? 'Resolving relationships' : 'Parsing source files',
      progress: { completed: Number(match[1]), total: Number(match[2]) } });
    if (line.includes('introspecting Cargo')) return this.update({ stage: 'Resolving workspace dependencies' });
  }

  async stop() {
    clearInterval(this.timer);
    this.timer = undefined;
    await this.update({ state: 'stopped', stage: 'Automatic indexing stopped' });
  }
}
