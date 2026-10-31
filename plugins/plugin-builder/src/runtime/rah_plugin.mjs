// RAH process plugin runtime (protocol 4), Node.js built-ins only.
//
// Written by plugin-builder next to `plugin.mjs`; it is replaced on every
// save, so do not edit it. It speaks newline-delimited JSON-RPC 2.0 on stdin
// and stdout, answers `initialize` with `manifest.json` from this directory
// and sends each request to the handler registered for it:
//
//     import { Plugin, InvalidInput } from "./rah_plugin.mjs";
//
//     const plugin = new Plugin(import.meta.url);
//     plugin.tool("my-plugin.greet", async (args, ctx) => {
//       if (typeof args.name !== "string") throw new InvalidInput("`name` must be a string");
//       return `Hello, ${args.name}!`;
//     });
//     plugin.run();
//
// Handlers may be async. They receive a context with `session`, `config`
// (the mount entry's `plugin` value), `callHost(service, method, params)` and
// `viewChanged(view)`. Diagnostics go to stderr; stdout is the protocol.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

const PROTOCOL = 4;

/** The input was refused and nothing was done: the host records no effect. */
export class InvalidInput extends Error {}

/** A `service/call` failed; `code` is the JSON-RPC error code. */
export class HostError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

class Refused extends Error {
  constructor(kind, message) {
    super(message);
    this.kind = kind;
  }
}

function toolResult(value) {
  if (value !== null && typeof value === "object" && Array.isArray(value.content)) {
    return value;
  }
  if (typeof value === "string") {
    return { content: [{ type: "text", text: value }] };
  }
  return { content: [{ type: "json", json: value ?? null }] };
}

export class Plugin {
  constructor(entryUrl) {
    const directory = dirname(fileURLToPath(entryUrl));
    this.manifest = JSON.parse(readFileSync(join(directory, "manifest.json"), "utf8"));
    this.config = null;
    this.tools = new Map();
    this.hooks = new Map();
    this.events = new Map();
    this.views = new Map();
    this.actions = new Map();
    this.pending = new Map();
    this.nextId = 0;
  }

  /** Registers `handler(args, ctx)` for a tool of the manifest. */
  tool(name, handler) {
    this.tools.set(name, handler);
  }

  /** Registers `handler(input, ctx)` for a hook of the manifest. */
  hook(name, handler) {
    this.hooks.set(name, handler);
  }

  /** Registers `handler(data, ctx)` for an event of the manifest. */
  event(name, handler) {
    this.events.set(name, handler);
  }

  /** Registers `render(ctx)` for a view; it returns an array of nodes. */
  view(id, render) {
    this.views.set(id, render);
  }

  /** Registers `handler(input, ctx)` for a button or form of a view. */
  action(view, action, handler) {
    this.actions.set(`${view}\u0000${action}`, handler);
  }

  send(message, done) {
    process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...message }) + "\n", done);
  }

  context(session) {
    return {
      session,
      config: this.config,
      callHost: (service, method, params = {}) => this.callHost(service, method, params),
      viewChanged: (view) => this.send({ method: "ui/changed", params: { view } }),
    };
  }

  callHost(service, method, params) {
    this.nextId += 1;
    const id = `rah-plugin-${this.nextId}`;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.send({ id, method: "service/call", params: { service, method, params } });
    });
  }

  check() {
    const m = this.manifest;
    const declared = {
      tool: new Set((m.tools ?? []).map((tool) => tool.name)),
      hook: new Set((m.hooks ?? []).map((hook) => hook.hook)),
      event: new Set(m.events ?? []),
      view: new Set((m.views ?? []).map((view) => view.id)),
    };
    const handled = { tool: this.tools, hook: this.hooks, event: this.events, view: this.views };
    const problems = [];
    for (const kind of Object.keys(declared)) {
      for (const name of declared[kind]) {
        if (!handled[kind].has(name)) problems.push(`${kind} \`${name}\` is in manifest.json but has no handler`);
      }
      for (const name of handled[kind].keys()) {
        if (!declared[kind].has(name)) problems.push(`${kind} \`${name}\` has a handler but is not in manifest.json`);
      }
    }
    for (const key of this.actions.keys()) {
      const [view, action] = key.split("\u0000");
      if (!declared.view.has(view)) problems.push(`action \`${action}\` belongs to unknown view \`${view}\``);
    }
    if (problems.length > 0) {
      for (const problem of problems) process.stderr.write(`rah_plugin: ${problem}\n`);
      process.exit(1);
    }
  }

  async handle(method, params) {
    const session = params.session ?? null;
    switch (method) {
      case "tools/call": {
        const handler = this.tools.get(params.name);
        if (!handler) throw new InvalidInput(`unknown tool \`${params.name}\``);
        return toolResult(await handler(params.arguments ?? {}, this.context(session)));
      }
      case "hooks/call": {
        const handler = this.hooks.get(params.hook);
        return handler ? ((await handler(params.input, this.context(null))) ?? null) : null;
      }
      case "views/render": {
        const render = this.views.get(params.view);
        if (!render) throw new Refused("unknown_view", `unknown view \`${params.view}\``);
        const nodes = await render(this.context(session));
        return Array.isArray(nodes) || nodes == null ? { nodes: nodes ?? [] } : nodes;
      }
      case "views/act": {
        const handler = this.actions.get(`${params.view}\u0000${params.action}`);
        if (!handler) throw new Refused("unknown_action", `unknown action \`${params.action}\``);
        return (await handler(params.input ?? null, this.context(session))) ?? {};
      }
      default:
        throw new Refused(null, `unknown method \`${method}\``);
    }
  }

  async answer(id, method, params) {
    try {
      this.send({ id, result: await this.handle(method, params) });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (error instanceof InvalidInput) {
        this.send({ id, error: { code: -32602, message, data: { kind: "invalid_input" } } });
      } else if (error instanceof Refused) {
        const fault = { code: -32601, message };
        if (error.kind) fault.data = { kind: error.kind };
        this.send({ id, error: fault });
      } else {
        this.send({ id, error: { code: -32000, message } });
      }
    }
  }

  receive(message) {
    const { id, method } = message;
    const params = message.params ?? {};
    if (method === undefined) {
      const waiting = this.pending.get(id);
      if (!waiting) return;
      this.pending.delete(id);
      if (message.error) {
        waiting.reject(new HostError(message.error.code ?? -32000, message.error.message ?? ""));
      } else {
        waiting.resolve(message.result);
      }
      return;
    }
    if (method === "initialize") {
      this.config = params.config ?? null;
      this.send({ id, result: { ...this.manifest, protocol: PROTOCOL } });
    } else if (method === "shutdown") {
      // Exit once the answer is written; pipes may be asynchronous.
      this.send({ id, result: {} }, () => process.exit(0));
    } else if (method === "event") {
      const handler = this.events.get(params.event);
      if (handler) {
        Promise.resolve()
          .then(() => handler(params.data, this.context(null)))
          .catch((error) => process.stderr.write(`rah_plugin: event handler failed: ${error}\n`));
      }
    } else if (id !== undefined && id !== null) {
      this.answer(id, method, params);
    }
    // Notifications such as `$/cancel` need no answer.
  }

  /** Serves the host until it shuts the plugin down. */
  run() {
    this.check();
    const lines = createInterface({ input: process.stdin });
    lines.on("line", (line) => {
      if (line.trim() !== "") this.receive(JSON.parse(line));
    });
    lines.on("close", () => {
      for (const waiting of this.pending.values()) {
        waiting.reject(new HostError(-32000, "the host closed the connection"));
      }
      process.exit(0);
    });
  }
}
