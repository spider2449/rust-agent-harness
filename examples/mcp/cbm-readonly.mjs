// Windows CBM v0.12.1 adapter for RAH's supervised stdio MCP transport.
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import * as fs from 'node:fs/promises';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

const execute = promisify(execFile);
const digest = '39f6a789cd858d7428561ac5afaefb9a47b018dcba0ad6fa41ae71a45e0ed656';
const project = 'rah-workspace';
const outputLimit = 48 * 1024;
const guidance = 'Index results may be stale. Treat resolver strategies and confidence as evidence. Verify low-confidence candidates such as unique_name against source.';
const textField = { type: 'string', minLength: 1, maxLength: 4096 };
const tools = {
  search_graph: ['Find symbols by text.', { query: textField }, ['query']],
  trace_path: ['Inspect candidate call paths and available resolver evidence.', {
    function_name: textField, direction: { enum: ['inbound', 'outbound', 'both'] },
    depth: { type: 'integer', minimum: 1, maximum: 3 },
  }, ['function_name']],
  get_code_snippet: ['Read a bounded snippet for an indexed symbol.', {
    qualified_name: textField, max_lines: { type: 'integer', minimum: 1, maximum: 80 },
  }, ['qualified_name']],
  query_graph: ['Run a query in CBM’s read-only Cypher subset.', { query: textField }, ['query']],
};

const canonical = value => {
  const normalized = path.normalize(value);
  return process.platform === 'win32' ? normalized.toLowerCase() : normalized;
};

export function queryArguments(name, input) {
  if (!Object.hasOwn(tools, name)) throw new Error('CBM tool is unavailable');
  if (!input || typeof input !== 'object' || Array.isArray(input)) throw new Error('Expected an arguments object');
  if (Object.hasOwn(input, 'project') && input.project !== project) throw new Error('Unknown CBM project');
  const [, properties, required] = tools[name];
  const args = {};
  for (const [key, value] of Object.entries(input)) {
    if (key === 'project') continue;
    const field = Object.hasOwn(properties, key) && properties[key];
    if (!field || (field.enum && !field.enum.includes(value))
        || (field.type === 'string' && (typeof value !== 'string' || !value.trim() || value.length > field.maxLength || value.includes('\0')))
        || (field.type === 'integer' && (!Number.isInteger(value) || value < field.minimum || value > field.maximum))) {
      throw new Error(`Invalid CBM argument: ${key}`);
    }
    args[key] = value;
  }
  if (required.some(key => !Object.hasOwn(args, key))) throw new Error('Missing required CBM argument');
  return { ...args, project, format: 'json', max_output_tokens: 3000,
    ...(name === 'search_graph' && { limit: 30 }),
    ...(name === 'trace_path' && { mode: 'calls', depth: args.depth ?? 2, limit: 30, include_evidence: true }),
    ...(name === 'get_code_snippet' && { max_lines: args.max_lines ?? 60 }),
    ...(name === 'query_graph' && { max_rows: 30 }) };
}

export function serve(input, output, read) {
  const lines = createInterface({ input, crlfDelay: Infinity });
  let active;
  const send = message => output.write(JSON.stringify({ jsonrpc: '2.0', ...message }) + '\n');
  lines.on('close', () => active?.controller.abort());
  lines.on('line', async line => {
    if (Buffer.byteLength(line) > 65536) return;
    let request;
    try { request = JSON.parse(line); } catch { return; }
    if (!request || typeof request !== 'object') return;
    const { id, method, params } = request;
    if (method === 'notifications/cancelled' && active && active.id === params?.requestId) {
      active.controller.abort();
      return;
    }
    if (id === undefined) return;
    const reply = result => send({ id, result });
    if (method === 'initialize') return reply({
      protocolVersion: '2024-11-05', capabilities: { tools: {} },
      serverInfo: { name: 'rah-cbm-readonly', version: '1' },
    });
    if (method === 'ping') return reply({});
    if (method === 'tools/list') return reply({ tools: Object.entries(tools).map(([name, [description, properties, required]]) => ({
      name, description: `${description} ${guidance}`,
      inputSchema: { type: 'object', properties, required, additionalProperties: false },
    })) });
    if (method !== 'tools/call') return send({ id, error: { code: -32601, message: 'Method unavailable' } });
    const refuse = message => reply({ isError: true, content: [{ type: 'text', text: message }] });
    if (active) return refuse('CBM is busy');
    const controller = new AbortController();
    active = { id, controller };
    try {
      const args = queryArguments(params?.name, params?.arguments);
      const result = await read(params.name, args, controller.signal);
      if (Buffer.byteLength(JSON.stringify(result)) > outputLimit) throw new Error('CBM response exceeds the output limit');
      return reply(result);
    } catch (error) {
      return refuse(String(error.message).slice(0, 512));
    } finally {
      active = undefined;
    }
  });
}

async function backend(workspace, prepare) {
  if (process.platform !== 'win32') throw new Error('This CBM integration pins the Windows x64 binary');
  const binary = process.env.RAH_CBM_BINARY;
  const home = process.env.RAH_CBM_HOME;
  if (!binary || !home || !path.isAbsolute(binary) || !path.isAbsolute(home)) {
    throw new Error('RAH_CBM_BINARY and RAH_CBM_HOME must be absolute paths');
  }
  const root = await fs.realpath(workspace);
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(binary)) hash.update(chunk);
  if (hash.digest('hex') !== digest) throw new Error('CBM binary does not match the pinned Windows v0.12.1 SHA-256');
  const key = createHash('sha256').update(canonical(root)).digest('hex').slice(0, 24);
  const directory = path.join(home, key);
  const cache = path.join(directory, 'cache');
  const runtime = path.join(directory, 'runtime');
  const receipt = path.join(directory, 'index.json');
  const env = { ...process.env, CBM_CACHE_DIR: cache, CBM_RUNTIME_DIR: runtime, CBM_ALLOWED_ROOT: root };
  async function run(args, signal, timeout = 8000) {
    try {
      return (await execute(binary, args, { cwd: root, env, signal, timeout,
        maxBuffer: outputLimit, windowsHide: true })).stdout;
    } catch (error) {
      if (error.code === 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER') throw new Error('CBM response exceeds the output limit');
      if (error.killed || signal?.aborted) throw new Error('CBM query timed out or was cancelled');
      if (args[2] === '--json' && error.stdout) {
        try {
          if (JSON.parse(error.stdout).isError === true) return error.stdout;
        } catch {
          // Preserve the command error if stdout was incomplete.
        }
      }
      throw new Error(`CBM command failed: ${String(error.stderr || error.code).trim().slice(0, 400)}`);
    }
  }
  if (prepare) {
    await fs.mkdir(cache, { recursive: true });
    await fs.mkdir(runtime, { recursive: true });
    for (const key of ['auto_index', 'auto_watch', 'watcher_enabled', 'ui_enabled']) {
      await run(['config', 'set', key, 'false']);
    }
    try {
      const outcome = JSON.parse(await run(['cli', '--quiet', 'index_repository', '--repo-path', root,
        '--name', project, '--mode', 'fast'], undefined, 600000));
      if (outcome.status !== 'indexed' || outcome.project !== project) throw new Error('CBM did not confirm indexing');
      const temporary = `${receipt}.${process.pid}.tmp`;
      try {
        await fs.writeFile(temporary, JSON.stringify({ root, project, indexed_at: new Date().toISOString() }), { flag: 'wx' });
        await fs.rename(temporary, receipt);
      } finally {
        await fs.rm(temporary, { force: true });
      }
    } finally {
      await run(['daemon', 'stop']);
    }
  }
  return async (name, args, signal) => {
    let recorded;
    try { recorded = JSON.parse(await fs.readFile(receipt, 'utf8')); }
    catch { throw new Error('CBM index is missing for the active workspace'); }
    if (recorded.project !== project || recorded.root !== root) {
      throw new Error('CBM index receipt belongs to a different workspace');
    }
    const listed = JSON.parse(await run(['cli', '--quiet', 'list_projects', '--format', 'json', '--limit', '2'], signal));
    const row = listed.projects?.[0];
    if (listed.total !== 1 || row?.name !== project || typeof row.root_path !== 'string'
        || await fs.realpath(row.root_path).catch(() => null) !== root) {
      throw new Error('CBM index ownership could not be verified');
    }
    const result = JSON.parse(await run(['cli', '--quiet', '--json', name, JSON.stringify(args)], signal));
    if (!Array.isArray(result.content)) throw new Error('CBM returned a malformed tool result');
    result.content.unshift({ type: 'text', text: `CBM v0.12.1 index prepared at ${recorded.indexed_at}. ${guidance}` });
    return result;
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [mode, root] = process.argv.slice(2);
  try {
    if (mode === '--prepare' && root && path.isAbsolute(root)) {
      await backend(root, true);
      process.stdout.write('CBM index prepared for the workspace.\n');
    } else if (!mode) {
      serve(process.stdin, process.stdout, await backend(process.cwd(), false));
    } else {
      throw new Error('Use --prepare with an absolute workspace path, or start the MCP adapter without arguments');
    }
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}
