# ADR 0047: API Groups as Sub-Modes of Daily Work

Status: Accepted

Date: 2026-10-17

## Context

Daily work needs special working setups such as "DDT": while one is wanted,
the model should be able to query that setup's own internal API, and only
that one. There are many such groups, each with its own endpoints and
credentials. ADR 0046 gives modes that switch plugins, but a mode names
plugin kinds, and a group is one instance of a generic API tool plugin.

## Decision

1. **`tools-http` declares API tools in configuration.** One instance is one
   group: `base_url`, `headers`, `header_env` and `tools`. A tool is one
   endpoint (`GET` or `POST`, a path with `{slot}`s, typed parameters) and
   declares `read` or `write`. Input is validated against the declared
   parameters before anything is sent; path values are percent-encoded;
   redirects are not followed; the response is bounded, has angle brackets
   replaced and is returned inside `<untrusted-api-response>`. Credentials
   come from environment variables (`header_env`), not from the
   configuration. The plugin is opt-in and the policy must assign each tool
   a decision (`decisions = { "ddt.*" = "allow" }`); like every tool it goes
   through `ToolInvoker`.
2. **A mode may extend another** (`extends`, one level). The sub-mode starts
   from its parent's plugins, instructions (the parent's first) and
   workspace, and its own settings override them. The sub-mode is resolved
   once when the modes are built, so the API, the WebSocket message and the
   UI see complete modes. The mode picker lists only top-level modes; the sub-modes of the selected mode are choices of the "Internal API" selector above the message box. Picking one connects that group, picking `Off` returns to the parent.
3. **A mode's `plugins` key may be an instance id**, not only a kind: it
   switches that one instance (`"http-ddt" = true`). An instance another
   mode runs and the entered mode does not list is stopped, including when
   the mode is cleared, so leaving a group removes its tools. Kinds keep
   their earlier behavior: nothing stops them implicitly. A key that is not
   a kind and names no instance is reported in `problems`.
4. **A `tools-http` instance no mode lists is an API group** and needs no
   mode. A mode with `groups = true` (the builtin daily work) shows one
   choice per group in the Web UI's "Internal API" selector (`Off` or one group, with a line saying what is connected) (`GET /api/groups`,
   `PUT /api/group {id, on}`). Pressing it starts the instance, pressing it
   again stops it. Only one group is connected at a time (pressing another
   disconnects the first), so the model never chooses between APIs. A group
   is independent of the active mode, and at start every group is stopped
   except the one that was on when the last run ended (kept in `storage`).
   The selector owns the instance's running state, so nothing has to be
   started in the Plugins panel: a group added or edited there while
   disconnected stays stopped until chosen, and one started by hand there
   replaces the connected group. Sub-modes (points 2 and 3) remain for a group that
   needs its own instructions.
5. **Selection is manual.** The user picks the sub-mode; the plugin never
   switches because a message mentions a keyword.

## Consequences

- A new group is configuration only: one `tools-http` instance, no code. A
  sub-mode is only needed to add instructions.
- While a group's mode is active, its tools are visible to the model for
  every session, as with any mode (ADR 0046, point 4). The policy still
  decides each call.
- A response is data from a server the configuration trusts, but its content
  is still untrusted text: the wrapper and the tool description tell the
  model so, and the policy governs what a model can do next.
- A `write` tool that loses its response leaves an `uncertain` effect and is
  never replayed (AGENTS.md).
