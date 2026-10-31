# Writing a personal RAH plugin

A personal plugin is a RAH **process plugin**: a small program that the user
mounts in `rah.toml`, which then talks to RAH over JSON-RPC on stdin and
stdout. You write its logic; `plugins.save` checks the manifest, adds the
runtime library and the mount instructions, and writes everything to the
user's plugin directory. Saving never mounts or runs the plugin. The user
reads the files and mounts it, and mounting is the user's trust decision.

## Workflow

1. Make sure you understand what the user wants. Ask about anything that
   changes the design: what the tools take and return, what must be kept
   between calls, whether it needs a view in the Web UI.
2. Choose a short name (lowercase letters, digits, `-`; it starts with a
   letter). Every tool name is `<name>.<tool>`.
3. Pick the smallest manifest that does the job, and the lowest permission
   for each tool.
4. Call `plugins.save`. On a refusal, fix what it names and save again.
5. Tell the user where the plugin is, what it can do, and to read its files
   and `mount.toml` before mounting. Do not claim it was tested: saving does
   not run it.

To change a plugin, `plugins.read` it, then save the whole plugin again with
`replace: true`. `plugins.list` lists the saved plugins.

## `plugins.save` input

```json
{
  "name": "weather",
  "description": "One line: what the plugin does.",
  "runtime": "python",
  "manifest": {
    "version": "0.1.0",
    "requires": [],
    "tools": [
      {
        "name": "weather.today",
        "description": "What the tool does, for the model that will call it.",
        "input_schema": {
          "type": "object",
          "properties": { "city": { "type": "string" } },
          "required": ["city"],
          "additionalProperties": false
        },
        "permission": "read"
      }
    ],
    "hooks": [],
    "events": [],
    "views": []
  },
  "files": [{ "path": "plugin.py", "content": "..." }],
  "replace": false
}
```

- `runtime` is `python` (Python 3, standard library only) or `node` (Node.js,
  built-in modules only). Do not use third-party packages: nothing installs
  them.
- `files` must contain the entry file, `plugin.py` for `python` or
  `plugin.mjs` for `node`. Other files are allowed (helpers, a README, data),
  in relative paths of letters, digits, `.`, `_` and `-`.
- `manifest.json`, `plugin-builder.json`, `mount.toml` and the runtime
  library `rah_plugin.py` / `rah_plugin.mjs` are written by `plugins.save`;
  do not send them.

## The manifest

- `version`: the plugin's version string.
- `tools`: at most 128. `name` is `<plugin name>.<tool>` without whitespace.
  `input_schema` is a JSON Schema with `"type": "object"`. `permission` is
  `none` (no effect outside the plugin), `read` (reads files, the network or
  other resources), `write` (changes something outside the plugin) or
  `execute` (runs programs). Declare honestly; the user's policy decides and
  can only be stricter.
- `requires`: host services the plugin calls, only `session` (read session
  logs) and `storage` (a key-value store of its own). The host refuses any
  service not listed.
- `hooks`: optional, one entry per hook, each `{"hook": ..., "priority": n}`:
  - `tool.before_invoke` sees each tool call and may deny it or ask the user.
    Its priority must be below `0`, so it runs after the policy and can never
    let through a call the policy refuses.
  - `tool.after_invoke` may replace a tool's output.
  - `model.before_request` may rewrite every model request.
  Hooks run on every call, have a 5 second limit, and a failing hook denies
  the call or fails the request. Use them only when the user asks for that
  behavior.
- `events`: optional, only `session.appended` (each new session record).
- `views`: optional panels in the Web UI, `{"id": "board", "title": "Board"}`,
  ids of lowercase letters, digits, `-` and `_`.
- `provides` and `protocol` are filled in by `plugins.save`.

## The runtime library

### Python (`plugin.py`)

```python
from rah_plugin import Plugin, InvalidInput

plugin = Plugin(__file__)

@plugin.tool("weather.today")
def today(args, ctx):
    city = args.get("city")
    if not isinstance(city, str) or not city.strip():
        raise InvalidInput("`city` must be a non-empty string")
    return {"city": city, "forecast": "sunny"}

plugin.run()
```

### Node.js (`plugin.mjs`)

```js
import { Plugin, InvalidInput } from "./rah_plugin.mjs";

const plugin = new Plugin(import.meta.url);

plugin.tool("weather.today", async (args, ctx) => {
  if (typeof args.city !== "string" || args.city.trim() === "") {
    throw new InvalidInput("`city` must be a non-empty string");
  }
  return { city: args.city, forecast: "sunny" };
});

plugin.run();
```

Node handlers may be async; Python handlers are plain functions. The names
below are the Python ones; Node uses `callHost` and `viewChanged`.

| Register | Handler | Returns |
|---|---|---|
| `tool(name)` | `(args, ctx)` | a string (text), any other JSON value, or `{"content": [...], "is_error": bool}` |
| `hook(name)` | `(input, ctx)` | `None` (no change) or the hook's result, see below |
| `event(name)` | `(data, ctx)` | nothing |
| `view(id)` | `(ctx)` | a list of view nodes |
| `action(view, action)` | `(input, ctx)` | `None` or `{"message": "...", "rerender": true}` |

Every manifest tool, hook, event and view needs exactly one handler, or the
plugin exits at start with the mismatch on stderr. `ctx` has:

- `ctx.session`: the calling session's id (tools, views, actions).
- `ctx.config`: the `plugin` value of the mount entry, for user settings.
- `ctx.call_host(service, method, params)`: a host service from `requires`:
  `session.list {}`, `session.read {"session": id}`, `storage.get {"key"}`
  (returns `{"value"}`, `None` when missing), `storage.put {"key", "value"}`,
  `storage.delete {"key"}`, `storage.list {"prefix"?}`. Keys are 1 to 128
  characters of letters, digits and `._:-`. It raises `HostError` on failure.
- `ctx.view_changed(view)`: windows showing the view render it again.

Hook results: `tool.before_invoke` gets `{session, call: {id, name, input},
definition}` and returns `None` or `{"decision": "deny", "reason": "..."}` /
`{"decision": "ask", "reason": "..."}`. `tool.after_invoke` gets `{session,
call, definition, output, effect}` and returns `None` or `{"output": <tool
result>}`. `model.before_request` gets `{model, items, tools}` and returns
`None` or `{"request": <request>}`.

View nodes: `{"type": "heading"|"text"|"markdown", "text"}`,
`{"type": "key_value", "items": [{"key", "value"}]}`,
`{"type": "table", "columns": [...], "rows": [[...]]}`,
`{"type": "list", "items": [...]}`, `{"type": "button", "action", "label"}`,
`{"type": "form", "action", "submit", "fields": [{"name", "label", "kind"?,
"value"?, "options"?}]}` with `kind` `text`, `number`, `boolean` or `choice`.
Values are shown as text, never as HTML. Actions run only on the user's
click; the model cannot reach views.

## Rules every plugin follows

- **Tool arguments come from the model and are untrusted.** Check every
  field's type, length and range; raise `InvalidInput` for bad input before
  doing anything (the host then records that nothing happened).
- Raise another exception for a failure after work started. The host never
  retries a call, and an interrupted `write` or `execute` call is recorded as
  uncertain, so do not retry effects silently either.
- Only stdout carries the protocol: never `print` to stdout. Write
  diagnostics to stderr.
- Keep state in `storage` (add it to `requires`), not in files next to the
  plugin; the process is restarted at any time.
- A tool call has 60 seconds. Bound loops, output size and network waits.
- Do not run shell commands or build command lines from model input. Do not
  read or write outside what the user asked for. Do not read secrets or send
  data anywhere the user did not name.
- Settings such as API keys come from `ctx.config` (the user puts them in the
  mount entry), never from the model and never written into the code.
