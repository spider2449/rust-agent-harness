# Process Plugins — Protocol v4

A process plugin is a RAH plugin implemented as a child process in any
language. The host is `rah-plugin-host-process`; the `rah` binary mounts one
for every `use = "process"` entry. Like every mounted plugin, a process plugin
is trusted code. The host supervises the process but does not sandbox it.

Three examples in Python (standard library only):
[`word_tools.py`](../examples/process-plugins/word_tools.py) provides tools
(protocol 2), [`tool_guard.py`](../examples/process-plugins/tool_guard.py)
attaches to hooks and follows session events (protocol 3), and
[`todo_board.py`](../examples/process-plugins/todo_board.py) adds a view to
the Web UI and keeps its items in `storage` (protocol 4).

Protocol 3 added hooks and events (ADR 0039); protocol 4 adds views
(ADR 0063). The host still accepts plugins that answer protocol 3 (no views)
or 2 (tools only).

## Mounting

```toml
[[plugin]]
use = "policy-authority"
config = { tools = { "words.*" = "read" } }

[[plugin]]
use = "process"
id = "word-tools"                  # required
config = { program = "python", args = ["examples/process-plugins/word_tools.py"] }
```

`config` fields:

| Field | Default | Meaning |
|---|---|---|
| `program`, `args` | required, `[]` | The command. No shell is involved. |
| `cwd` | `rah`'s directory | Working directory of the child. |
| `inherit_env` | `PATH` and a few platform variables | Variables copied from `rah`'s environment; everything else is cleared. |
| `env` | `{}` | Extra variables. `RAH_PLUGIN_PROTOCOL=4` is always set. |
| `plugin` | `null` | Passed to the plugin as `initialize.config`. |
| `startup_timeout_ms` | 10000 | Limit for `initialize`. |
| `call_timeout_ms` | 60000 | Limit for each `tools/call`. |
| `hook_timeout_ms` | 5000 | Limit for each `hooks/call`. |
| `view_timeout_ms` | 5000 | Limit for each `views/render` and `views/act`. |
| `max_message_bytes` | 4 MiB | Limit for one message in either direction. |

At boot `rah` starts the plugin once to read its manifest, then shuts it down.
Each later start (mount, restart after a dependency returns, configuration
update) starts a fresh process and fails if the manifest's services differ
from the one read at boot. The process belongs to the plugin's scope: on
unmount the host asks it to `shutdown` (best effort) and kills it when the
scope is disposed. When the plugin closes the connection on its own, the host
waits up to 500 ms for it to exit before killing it.

## Transport

Newline-delimited JSON-RPC 2.0 on stdin and stdout: one JSON object per line,
UTF-8, no batches. Anything on stdout that is not such a message is a
protocol violation and ends the connection. Write diagnostics to stderr; the
host keeps the last 16 KiB and includes them in startup errors.

## Host to plugin

### `initialize` (request)

```json
{"protocol": 4, "kernel_api": 1, "plugin_id": "word-tools", "config": null}
```

Result, the plugin's manifest:

```json
{
  "protocol": 4,
  "version": "0.1.0",
  "provides": ["tools", "ui-views"],
  "requires": ["session"],
  "optional": [],
  "tools": [
    {"name": "words.count", "description": "...",
     "input_schema": {"type": "object"}, "permission": "none"}
  ],
  "hooks": [{"hook": "tool.before_invoke", "priority": 10}],
  "events": ["session.appended"],
  "views": [{"id": "board", "title": "To-do"}]
}
```

- `protocol` must be `4`, `3` for a plugin without `views`, or `2` for a
  plugin without `views`, `hooks` and `events`.
- `provides` may contain only `tools` and `ui-views`; `tools` must be
  non-empty exactly when it contains `tools` (at most 128 tools, unique names
  without whitespace).
- `views` must be non-empty exactly when `provides` contains `ui-views`: at
  most 32, ids unique, lowercase letters, digits, `-` and `_` (at most 64
  bytes). The Web UI shows each as a panel `<plugin id>/<id>` with `title`
  as its tab label.
- `requires` and `optional` may contain only `session` and `storage`. A required service
  must be mounted before the plugin starts; `optional` services cannot be
  called.
- `permission` is one of `none`, `read`, `write`, `execute`. It is the
  plugin's declaration; `policy-authority` still needs its own assignment and
  can only be made stricter by it.
- `hooks` lists at most one entry per hook, of `tool.before_invoke`,
  `tool.after_invoke` and `model.before_request` (see below). `priority`
  defaults to `0`; higher runs first. `policy-authority` uses `0`.
- `events` may contain `session.appended`.
- A plugin may have no tools and only hooks, events or views.

### `tools/call` (request)

```json
{"name": "words.count", "arguments": {"text": "a b"}, "session": "<session id>"}
```

Result:

```json
{"content": [{"type": "text", "text": "..."}, {"type": "json", "json": {}}], "is_error": false}
```

A content item may also be an image for models that take images:
`{"type": "image", "media_type": "image/png", "data": "<base64>"}`. The media
type must be `image/png`, `image/jpeg`, `image/gif` or `image/webp`, and the
data padded standard base64 of at most 4 MiB (no `data:` prefix). Any other
image makes the whole result malformed. Keep `max_message_bytes` large
enough for it.

`arguments` come from the model and are untrusted; validate them. An error
response whose `data.kind` is `invalid_input` or `not_started` tells the host
that nothing was done (effect `none`). Any other error, a timeout, a
cancellation or a lost connection leaves the effect of a `write` or `execute`
tool `uncertain`; the host never replays a call.

### `hooks/call` (request)

```json
{"hook": "tool.before_invoke", "input": {}}
```

Sent for each hook in the manifest, limited by `hook_timeout_ms`. Never
retried.

| Hook | `input` | Result |
|---|---|---|
| `tool.before_invoke` | `{session, call: {id, name, input}, definition}` | `null` (no decision) or `{"decision": "allow"}`, `{"decision": "deny", "reason"}`, `{"decision": "ask", "reason"}` |
| `tool.after_invoke` | `{session, call, definition, output, effect}` | `null` (unchanged) or `{"output": <tool output>}` |
| `model.before_request` | the model request `{model, items, tools}` | `null` (unchanged) or `{"request": <model request>}` |

- `call.input` and the request's items come from the model and are
  untrusted.
- `output` uses the same form as a `tools/call` result and is checked the
  same way. Only the output can change; `effect` is the tool's and stays.
- A model request uses RAH's own JSON form, the same as session records: tool
  content items are `{"type": "text", "value": "..."}`, `{"type": "json",
  "value": ...}` or `{"type": "image", "value": {"media_type", "data"}}`.
  Returned images are checked. The whole request crosses the pipe, so
  `max_message_bytes` must hold it.
- `ask` needs an approver (a frontend); without one the call is denied.
- A plugin with a higher priority than `policy-authority` decides first. An
  `allow` from it lets the call through without the policy. That is the
  user's decision when mounting it.

A timeout, cancellation, error response, malformed result or lost
connection behaves like a failing Rust handler:

- `tool.before_invoke`: the call is denied, with the failure as the reason.
- `tool.after_invoke`: the output becomes an error naming the failure.
- `model.before_request`: the model request fails and the turn ends with an
  error. The kernel reports the failure as a panicking handler.

### `views/render` (request)

```json
{"view": "board", "session": "<session id or null>"}
```

Result: the view, `{"nodes": [...]}`. `session` is the session open in the
window. Nodes are objects with a `type`:

| `type` | Fields |
|---|---|
| `heading`, `text` | `text` |
| `markdown` | `text`; no raw HTML, no images, only `http(s)` and `mailto` links |
| `key_value` | `items`: `[{key, value}]` |
| `table` | `columns`: `[string]`, `rows`: `[[string]]` |
| `list` | `items`: `[string]` |
| `button` | `action`, `label` |
| `form` | `action`, `submit` (button label), `fields`: `[{name, label, kind?, value?, options?}]`, `kind` one of `text` (default), `number`, `boolean`, `choice` |

An unknown type or field makes the view malformed. The page builds a view
with text only, never as HTML, and refuses one over 2000 nodes (each table
cell, list item and field counts) or 256 KiB.

### `views/act` (request)

```json
{"view": "board", "action": "add", "input": {"item": "milk"}, "session": null}
```

The user pressed a button (`input` is `null`) or submitted a form (`input` is
an object of the fields' values: strings, numbers or `null` for an empty
number, and booleans). Result: `{"message"?: "<one line>", "rerender"?:
true}`. Actions run only on the user's click; the model cannot reach views.

An error whose `data.kind` is `unknown_view`, `unknown_action` or
`invalid_input` is shown as such (the HTTP API answers 404 or 400). Any other
error, a timeout or a malformed result is shown as a failure. An action that
timed out may still have happened; the host never repeats it.

### `event` (notification)

```json
{"event": "session.appended", "data": {"session": "<id>", "seq": 3, "entry": {}}}
```

Sent for each event in the manifest. `entry` is a session record entry, as
from `session.read`. Delivery is best effort, in order and at most once.
Events are lost while the plugin is not running. Use `seq` and
`session.read` to catch up.

### `$/cancel` (notification)

`{"id": <request id>}`: the host gave up on that request (timeout or
cancellation). Later answers to it are ignored.

### `shutdown` (request)

Answer `{}` and exit. The host closes stdin afterwards.

## Plugin to host

### `ui/changed` (notification)

```json
{"view": "board"}
```

A view in the manifest changed. The host emits `UiChanged`, and every Web UI
window showing the view renders it again. Other views are ignored.

### `service/call` (request)

```json
{"service": "session", "method": "read", "params": {"session": "<session id>"}}
```

The host refuses a service that is not in the plugin's own `requires` with
error code `-32003`, even when that service is mounted. Available methods:

| Service | Method | Params | Result |
|---|---|---|---|
| `session` | `list` | none | `[SessionInfo]` |
| `session` | `read` | `{session}` | `[SessionRecord]` (the append-only log) |
| `storage` | `get` | `{key}` | `{value}`, `null` when not stored |
| `storage` | `put` | `{key, value}` (any JSON) | `{}` |
| `storage` | `delete` | `{key}` | `{deleted}` |
| `storage` | `list` | `{prefix?}` | `{keys}`, sorted |

`storage` (ADR 0041) keeps entries in the namespace of the plugin's id; a
plugin cannot reach another plugin's entries. Keys are 1 to 128 bytes of
letters, digits and `._:-`. An invalid key or parameter is error `-32602`,
an unknown method `-32601`, a storage failure `-32000`. A distribution must
mount a provider, such as `storage-fs` (every built-in preset does).

The host answers plugin requests concurrently with its own; a plugin may call
a service while it handles a `tools/call` or a `hooks/call`.

## Not yet available

Providing services other than `tools` and `ui-views`, emitting events
other than `ui/changed`, and the
`tool.approval` and `host.interrupt` hooks. For capability-isolated plugins
see [WASM plugins](WASM_PLUGINS.md).
