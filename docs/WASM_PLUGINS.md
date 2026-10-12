# WASM Plugins

A WASM plugin is a WebAssembly component that implements the WIT world
`rah:plugin/wasm-plugin`. The host is `rah-plugin-host-wasm` (wasmtime); the
`rah` binary mounts one for every `use = "wasm"` entry. Unlike a
[process plugin](PROCESS_PLUGINS.md), a WASM plugin is **capability-isolated**:
it runs inside the host process with no WASI, so it cannot open files, use the
network, read the clock, the environment or randomness. Its only way out is
`call-service`, limited to the services in its own `requires`.

## Mounting

```toml
[[plugin]]
use = "policy-authority"
config = { tools = { "wasm.*" = "read" } }

[[plugin]]
use = "wasm"
id = "my-wasm"                         # required
config = { path = "plugins/my_plugin.wasm", fuel = 100000000, max_memory_mb = 64 }
```

| Field | Default | Meaning |
|---|---|---|
| `path` | required | The component, in binary or text format. |
| `fuel` | 100 000 000 | Instruction budget for each call; exhausting it traps the call. |
| `max_memory_mb` | 64 | Linear memory cap for each call. |

`rah` compiles the component and reads its manifest once at boot. Every tool
call instantiates the component in a fresh store, so a guest keeps no state
between calls and a trap affects only that call.

## The WIT world

```wit
package rah:plugin@0.1.0;

interface host {
  call-service: func(service: string, method: string, params: string) -> result<string, string>;
}

world wasm-plugin {
  import host;
  export manifest: func() -> string;
  export call-tool: func(name: string, input: string) -> result<string, string>;
}
```

### `manifest`

Returns JSON:

```json
{
  "version": "0.1.0",
  "requires": ["session"],
  "tools": [
    {"name": "wasm.count", "description": "...",
     "input_schema": {"type": "object"}, "permission": "none"}
  ]
}
```

A WASM plugin provides `tools` (1 to 128, unique names without whitespace)
and may require only `session`. Permissions are its declaration;
`policy-authority` still assigns each tool and can only be made stricter by
it.

### `call-tool`

`input` is the model's JSON arguments (untrusted). `ok(text)` becomes the
tool output. `err(text)` is a failure: when `text` starts with
`invalid_input:` nothing was done (effect `none`); any other error, or a trap,
leaves the effect of a `write` or `execute` tool `uncertain`. Calls are never
replayed.

### `host.call-service`

| Service | Method | Params | Result |
|---|---|---|---|
| `session` | `list` | `{}` | JSON `[SessionInfo]` |
| `session` | `read` | `{"session": "<id>"}` | JSON `[SessionRecord]` |

Any service outside the plugin's `requires` returns an error, even when that
service is mounted.

## Building a component

Any toolchain that produces a component for this world works, for example
Rust with `wit-bindgen` and the `wasm32-wasip2` target, or a component in
text format as in `crates/rah-plugin-host-wasm/tests/wasm.rs`. The component
must not import WASI or anything other than `rah:plugin/host`; such a
component fails to instantiate.

## Limits

Fuel and memory bound every call. A call that its caller cancels is
interrupted (wasmtime epoch interruption): the guest traps at its next epoch
check, normally within microseconds, and releases its blocking thread; the
result is discarded. Other calls of the same plugin are not affected. A
guest blocked inside `call-service` is interrupted once that host call
returns.
