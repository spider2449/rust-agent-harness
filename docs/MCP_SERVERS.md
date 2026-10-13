# Connecting MCP Servers

RAH connects to an MCP server through the `tools-mcp` plugin. Each server is
one more `[[plugin]]` entry in a distribution file. You do not need a new
crate or any Rust code. This page covers the general steps, then walks
through Blender as a worked example
([`examples/mcp/blender.toml`](../examples/mcp/blender.toml)).

## How it fits together

```
model -> loop -> ToolInvoker -> tool.before_invoke (policy-authority)
                     |
                     v
            tools-mcp instance --(MCP over stdio)--> server process
                     |                                   ^
                     +-- spawned through `sandbox` ------+
```

- `tools-mcp` speaks the MCP **stdio** transport only. It starts the server
  with the `sandbox` service, so the server gets a cleared environment and a
  working directory inside the sandbox root. The process tree ends when the
  plugin is unmounted. This is process supervision, not an OS sandbox: the
  server keeps network and filesystem access.
- Only the tools listed in the plugin's `tools` table are exposed. Each one
  is named `mcp.<server>.<remote name>` and declares the permission level
  written there. The server's own metadata (`readOnlyHint` and similar
  annotations) grants nothing.
- `policy-authority` must also assign every exposed name. A tool with no
  assignment is denied. The effective level is the stricter of the policy
  assignment and the declared level.
- A call that times out, is cancelled or loses the connection is never
  replayed. Under `write` or `execute`, its effect is recorded as
  `uncertain`. A crashed server stays down until RAH restarts.

## Adding a server

1. **Find the stdio command.** Use the command the server's documentation
   gives for MCP clients (for example `uvx <package>`, `npx <package>` or a
   binary). If the server only speaks HTTP or SSE, `tools-mcp` cannot
   connect to it directly.
2. **List its tools.** Tool names must match exactly. A listed tool that the
   server does not offer stops the plugin from starting. Mount the server
   with one tool you are sure of, and check the error message or the
   server's documentation for the rest.
3. **Assign a level per tool.** Use `read` for queries, `write` for state
   changes and `execute` for arbitrary code. Leave out tools you do not
   need. Under the preset rules, `none` and `read` run directly, and `write`
   and `execute` ask on the terminal before each call.
4. **Add the entries.** Add a `tools-mcp` entry with a unique `id` and
   `server`. Make sure `sandbox-process` is mounted. Assign the names in
   `policy-authority`, using a `mcp.<server>.*` prefix and exact names
   wherever a tool needs a stricter level.
5. **Run it:** `cargo run -p rah-cli -- --config <file>`. Note that
   `--config` replaces the preset, so the file must list every plugin.

`tools-mcp` configuration fields: `server`, `program`, `args`, `tools`
(required); `cwd`, `env`, `call_timeout_ms` (30 s), `startup_timeout_ms`
(10 s), `max_message_bytes` (1 MiB), `max_outstanding` (16). See the module
documentation in `plugins/tools-mcp/src/lib.rs`.

## Example: Blender

The [Blender Lab MCP server](https://www.blender.org/lab/mcp-server/) has two
parts:

```
RAH --(MCP, stdio)--> blender-mcp --(JSON + NUL byte, TCP localhost:9876)--> Blender "MCP" add-on
```

**Port 9876 is not an MCP endpoint.** The Blender add-on listens on it and
runs the Python code that the `blender-mcp` server sends. RAH starts
`blender-mcp`, and `blender-mcp` connects to the port. Set `BLENDER_MCP_HOST`
and `BLENDER_MCP_PORT` in the plugin's `env` to change the address.

### Setup

1. Use Blender 5.1 or later. Add the extensions repository
   `https://lab.blender.org/`, then install and enable the **MCP** add-on
   (v1.0.3 was tested). The add-on needs Online Access enabled in
   Preferences, and it then listens on `localhost:9876`.
2. Install [`uv`](https://docs.astral.sh/uv/) and make sure `uvx` is on
   `PATH`. The sandbox's default inherited environment is enough for it.
3. Run from the repository root:

   ```powershell
   cargo run -p rah-cli -- --config examples/mcp/blender.toml
   ```

   The first start downloads and builds the server, which can take about
   30 seconds. That is why the example sets `startup_timeout_ms = 180000`.

The example mounts `model-llamacpp`, which uses a local llama-server on
`http://127.0.0.1:8080`, the same as the `default` preset. Ask in plain
language, for example "Which collections are in this scene and how many
objects does each hold?". The model picks the tools. Under `execute`, you
see the code and approve it before it runs. To use a hosted model instead,
swap in `model-anthropic` or `model-openai`.

To call tools without a model, for example to check the connection, swap in
`model-mock` and pass the request with `--prompt`:

```powershell
cargo run -p rah-cli -- --config examples/mcp/blender.toml --prompt '/call mcp.blender.get_objects_summary {}'
```

Windows PowerShell 5.1 drops the double quotes inside an argument it passes
to a native program, so escape them:
`'/call mcp.blender.get_object_detail_summary {\"name\":\"Camera\"}'`. Use
`--prompt` rather than typing `/call` in the REPL (see
[KNOWN_ISSUES.md](KNOWN_ISSUES.md)).

### Pick the right package

PyPI's `blender-mcp` (`uvx blender-mcp`) is a **different project**. It
uses its own Blender add-on and an incompatible socket protocol. Pointed at
the Blender Lab add-on, it starts but cannot talk to Blender. The example
installs the Blender Lab server from its repository instead, pinned to a
reviewed commit:

```
uvx --from "git+https://projects.blender.org/lab/blender_mcp.git@<commit>#subdirectory=mcp" blender-mcp
```

Update the pin on purpose, and review the tool list again when you do.

### Tools and levels

The example exposes these tools:

| Level | Tools |
|---|---|
| `read` | `get_objects_summary`, `get_object_detail_summary`, `get_blendfile_summary_*` (datablocks, missing_files, of_linked_libraries, path_info, usage_guess), `get_screenshot_of_window_as_json`, `get_python_api_docs`, `search_api_docs`, `search_manual_docs` |
| `write` | `jump_to_tab_by_name`, `jump_to_tab_by_space_type`, `jump_to_view3d_object_by_name`, `jump_to_view3d_object_data_by_name` (change the Blender UI) |
| `execute` | `execute_blender_code` |

These tools are left out on purpose:

- `*_for_cli`: they start a background Blender process and need
  `BLENDER_PATH`.
- `get_screenshot_of_area_as_image` and `get_screenshot_of_window_as_image`:
  `tools-mcp` passes image content to the model as JSON (base64). A
  screenshot easily exceeds `max_message_bytes` and fills the context.
- `render_thumbnail_to_path` and `render_viewport_to_path`: they write to
  any `output_path`, outside the sandbox root.

### Safety notes

- `execute_blender_code` runs arbitrary Python inside your Blender, with
  Blender's full access to files and the network. Keep it at `execute` so
  you approve every call, and never move it to `allow`. Blender Lab itself
  recommends a machine without sensitive data.
- Any local program can connect to the add-on's port while it is open. RAH
  does not control that.
- The `read` level reflects what a tool is meant to do. A Blender tool still
  runs Python inside Blender, so trust in the server and the add-on is part
  of mounting it.
- A timed-out `execute_blender_code` call may still have run in Blender.
  RAH records it as `uncertain` and does not retry it. Check Blender's state
  before you ask again.
