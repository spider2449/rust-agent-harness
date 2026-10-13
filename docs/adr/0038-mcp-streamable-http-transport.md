# ADR 0038: MCP Streamable HTTP Transport

Status: Accepted

Date: 2026-10-13

## Context

`tools-mcp` speaks MCP only over stdio, to a server it starts through the
`sandbox` service. Many MCP servers are only reachable over HTTP: hosted
services, servers inside another application, or servers that a container
runs. The legacy MCP adapter deferred Streamable HTTP (ADR 0007, item 11).

An HTTP server differs from a stdio server in three ways that matter for
safety:

- RAH does not start it, so there is no process for the sandbox to supervise.
- Tool arguments, which come from the model, travel over the network to an
  address. Which address is a trust decision.
- A token may have to be sent with every request.

## Decision

1. **A separate plugin kind.** The same crate provides `tools-mcp-http`
   beside `tools-mcp`. It does not require `sandbox`, because it starts no
   process. Everything above the transport is shared: listed tools only,
   names `mcp.<server>.<tool>`, permission levels from the configuration,
   ignored server annotations, image handling (ADR 0037) and output mapping.
   The policy must still assign the exposed names.

2. **Endpoint.** `url` comes only from the user's configuration, never from a
   model, a tool result or server metadata.
   - `https://` is allowed for any host.
   - `http://` is allowed only for a loopback host (`localhost`,
     `127.0.0.0/8`, `::1`).
   - Userinfo and fragments are rejected. Path and query are kept as given.
   - Redirects are not followed, and proxies from the environment are not
     used. An `https` endpoint is HTTPS-only, so a downgrade fails.

3. **Credentials.** Optional `bearer_token_env` names an environment
   variable. Its value is read once when the plugin starts and sent as
   `Authorization: Bearer …` to the configured endpoint only. It never
   appears in errors, logs, Debug output or the session log. A missing or
   empty variable fails the plugin. OAuth authorization flows are out of
   scope.

4. **Protocol.** Only Streamable HTTP is supported (protocol versions
   2025-03-26 and 2025-06-18). The legacy HTTP+SSE transport (2024-11-05) is
   not.
   - Each JSON-RPC message is one `POST` with
     `Accept: application/json, text/event-stream`.
   - A response is either one JSON body or an SSE stream that ends with the
     response. Server requests on that stream get the same answers as over
     stdio: `ping` is answered and everything else is refused.
   - The `Mcp-Session-Id` from `initialize` is sent on every later request.
     After initialization, `MCP-Protocol-Version` is sent too.
   - No `GET` stream is opened, so server-initiated messages outside a
     request are not received.
   - On dispose, a `DELETE` ends the session (best effort, bounded by a short
     timeout).

5. **Failures and replay.** The no-replay rule applies unchanged.
   - A connection that could not be made records no effect.
   - A timeout, a cancellation, or a stream that ends without the response
     leaves the effect of a `write` or `execute` tool `uncertain`. A
     cancellation also sends `notifications/cancelled`.
   - An HTTP error status is a tool error. Its effect is decided as for any
     other error.
   - `404` on a request with a session id means the server ended the
     session. The plugin reports it and does not initialize again on its
     own; restarting the plugin starts a new session.
   - Stream resumption (`Last-Event-ID`) is not used.

6. **Bounds.** `max_message_bytes` (default 8 MiB) limits a JSON body and
   each SSE event. `max_outstanding`, `call_timeout_ms` and
   `startup_timeout_ms` keep their stdio meaning.

7. **Dependency.** `rah-plugin-tools-mcp` gains `reqwest`, with the same
   features as the model plugins (`rustls`, `stream`). No new crate enters
   the workspace.

## Consequences

- MCP servers that only speak HTTP can be mounted. A remote endpoint
  receives model-chosen tool arguments, and that is the user's decision when
  writing the configuration.
- Process supervision does not cover these servers. Their effects are as
  trustworthy as the server.
- Servers that need OAuth, or that push messages outside requests, are not
  supported yet.
