# Process Plugins — Protocol v2

A process plugin is a RAH plugin implemented as a child process in any
language. The host is `rah-plugin-host-process`; the `rah` binary mounts one
for every `use = "process"` entry. Like every mounted plugin, a process plugin
is trusted code. The host supervises the process but does not sandbox it.

A complete example in Python (standard library only) is
[`examples/process-plugins/word_tools.py`](../examples/process-plugins/word_tools.py).

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
| `env` | `{}` | Extra variables. `RAH_PLUGIN_PROTOCOL=2` is always set. |
| `plugin` | `null` | Passed to the plugin as `initialize.config`. |
| `startup_timeout_ms` | 10000 | Limit for `initialize`. |
| `call_timeout_ms` | 60000 | Limit for each `tools/call`. |
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
{"protocol": 2, "kernel_api": 1, "plugin_id": "word-tools", "config": null}
```

Result, the plugin's manifest:

```json
{
  "protocol": 2,
  "version": "0.1.0",
  "provides": ["tools"],
  "requires": ["session"],
  "optional": [],
  "tools": [
    {"name": "words.count", "description": "...",
     "input_schema": {"type": "object"}, "permission": "none"}
  ]
}
```

- `protocol` must be `2`.
- `provides` may contain only `tools`; `tools` must be non-empty exactly when
  it does (at most 128 tools, unique names without whitespace).
- `requires` and `optional` may contain only `session`. A required service
  must be mounted before the plugin starts; `optional` services cannot be
  called.
- `permission` is one of `none`, `read`, `write`, `execute`. It is the
  plugin's declaration; `policy-authority` still needs its own assignment and
  can only be made stricter by it.

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

### `$/cancel` (notification)

`{"id": <request id>}`: the host gave up on that request (timeout or
cancellation). Later answers to it are ignored.

### `shutdown` (request)

Answer `{}` and exit. The host closes stdin afterwards.

## Plugin to host

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

The host answers plugin requests concurrently with its own; a plugin may call
a service while it handles a `tools/call`.

## Not yet available

Hooks (for example a `tool.before_invoke` policy in another language),
providing services other than `tools`, and events. For capability-isolated plugins see [WASM plugins](WASM_PLUGINS.md).
