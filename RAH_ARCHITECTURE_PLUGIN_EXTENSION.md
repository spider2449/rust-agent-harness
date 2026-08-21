# RAH Architecture — Plugin Extension Point Addendum

Status: v0.1 architecture constraint realized by focused v0.2 adapter prototypes
Scope: historical v0.1 design context and remaining general-platform extensibility

## 1. Plugin positioning

RAH must preserve a plugin extension point from v0.1 onward, but v0.1 does not implement a general-purpose plugin platform.

The first supported plugin model is:

```text
External Tool Provider
        |
        v
 Plugin Adapter / MCP Adapter
        |
        v
    Tool trait
        |
        v
   ToolRegistry
        |
        v
   AgentRuntime
```

The AgentRuntime must not distinguish between:

```text
Built-in Tool
MCP Tool
Process Plugin Tool
```

All of them must converge into the same RAH-owned `Tool` abstraction.

## 2. Core rule

The plugin boundary is a tool boundary.

A plugin must not directly mutate or own:

- AgentContext;
- Session internals;
- AgentRuntime internals;
- ModelBackend internals;
- policy engine internals;
- sandbox internals;
- event bus internals.

A plugin may expose tools whose calls are mediated by RAH.

Conceptually:

```text
Model
  -> ToolCall
  -> ToolRegistry
  -> Policy / Permission
  -> PluginToolProxy
  -> external plugin process
  -> ToolOutput
```

Plugins do not bypass policy or sandbox rules.

## 3. Preferred plugin transport

Do not design v0.1 around dynamic Rust library loading (`.dll`, `.so`, `.dylib`).

The v0.2 process-plugin prototype uses the preferred process boundary:

```text
RAH
  -> child process / IPC
  -> plugin
```

The implemented process-plugin prototype uses stdio JSON-RPC. Other future
process-plugin transports may include:

- local socket / named pipe where justified.

MCP uses its own RAH adapter and converges at the `Tool` boundary.

This keeps plugins language-agnostic and avoids Rust ABI stability constraints.

Possible plugin languages include:

```text
Rust
Python
Node.js
Go
other languages capable of the chosen protocol
```

## 4. Plugin manifest direction

A future plugin manifest may describe:

```text
identity
version
runtime command
runtime arguments
declared tools
required permissions
protocol version
capabilities
```

Illustrative only:

```toml
[plugin]
name = "image-tools"
version = "0.1.0"

[runtime]
command = "python"
args = ["plugin.py"]

[permissions]
filesystem = "read"
network = false
process = false
```

The exact manifest schema is deferred.

## 5. Plugin lifecycle direction

The focused v0.2 process-plugin lifecycle is:

```text
host configuration
 -> spawn
 -> handshake and validate identity
 -> discover tools
 -> permission check
 -> register tools
 -> execute tool calls
 -> shutdown
```

RAH owns lifecycle supervision.

Plugins must not assume they remain alive forever.

## 6. MCP relationship

MCP should be treated as an early external-tool extension mechanism.

Conceptually:

```text
ToolRegistry
   |
   +-- Built-in Tool
   +-- MCP Tool
   +-- Process Plugin Tool
```

RAH must avoid building a second incompatible tool model for plugins.

MCP integration and custom plugin integration should adapt into the same `Tool` interface.

## 7. Security invariant

A plugin declaration is not permission.

A plugin tool call still follows:

```text
ToolCall
 -> ToolRegistry
 -> Permission / Policy
 -> Plugin execution boundary
 -> ToolOutput
```

Plugin permissions are explicit, host-owned, default-deny, and should remain
least-privilege.

The plugin process must not implicitly inherit full RAH authority.

## 8. API stability rule

RAH public APIs must not expose plugin-runtime implementation types.

Examples of types that must remain RAH-owned and neutral:

```text
ToolDefinition
ToolCall
ToolInput
ToolOutput
PermissionLevel
ToolError
```

This allows plugin transports to change without changing AgentRuntime APIs.

## 9. v0.1 implementation impact

v0.1 does not need to implement:

- PluginManager;
- PluginManifest;
- plugin discovery;
- dynamic library loading;
- plugin marketplace;
- plugin installation;
- plugin SDK.

However, v0.1 implementation must not block these future additions.

In particular:

1. `ToolRegistry` must accept tools through trait objects or an equivalent extensible mechanism.
2. Tool identity must not assume compile-time-only tools.
3. Tool definitions must be serializable/provider-neutral.
4. Permission checks must be outside individual tool implementations where practical.
5. AgentRuntime must depend on tool abstractions, not concrete built-in tool types.
6. MCP tools and process-plugin tools enter through the same registry boundary.

## 10. Historical future roadmap

Deferred post-v0.1 work may include:

```text
Task 026 PluginManifest
Task 027 PluginProtocol
Task 028 PluginProcess
Task 029 PluginToolProxy
Task 030 PluginManager
Task 031 Plugin permissions
Task 032 Plugin lifecycle
Task 033 Plugin conformance tests
```

These were reserved v0.1 extension points, not authorization to implement them
during v0.1. v0.2 now contains focused MCP and process-plugin adapter prototypes;
a general `PluginManager`, manifest ecosystem, marketplace, installer, SDK, and
additional transports remain deferred.

## 11. Architecture invariant summary

The following must remain true:

```text
AgentRuntime
    |
    v
ToolRegistry
    |
    +-- Built-in Tool
    +-- MCP Tool
    +-- Process Plugin Tool
```

And never:

```text
Plugin
  -> mutate AgentRuntime internals
  -> bypass ToolRegistry
  -> bypass policy
  -> bypass sandbox
```

RAH plugins extend capabilities through tools.

They do not redefine the core runtime.
