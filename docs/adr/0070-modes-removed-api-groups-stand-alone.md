# ADR 0070: Modes Are Removed; API Groups Stand Alone

Status: Accepted

Date: 2026-11-10

## Context

ADR 0046 gave `frontend-web` modes (the built-in `daily` and `coding`, or
configured ones). A mode started and stopped plugins, added instructions to
every model request, hid the workspace and chose which plugin views showed.
ADR 0047 added sub-modes for API groups, and ADR 0062 let a mode list views.

In use this was confusing. One picker changed four unrelated things at once,
for every window, and each of them already had or later got its own control:

- web search is a feature with its own switch (ADR 0067, ADR 0069), kept in
  `rah.toml`, and the write tools are governed by the permissions setting
  and the Plugins panel, while a mode switched the same plugins at runtime
  without keeping it, so the two disagreed after a switch;
- instructions belong to the assistant settings and the instruction files;
- the side panel and its tabs are already opened and closed by the user;
- API groups had two forms: the selector and sub-modes.

## Decision

1. **Modes are removed.** `frontend-web` no longer has the `mode` and `modes`
   configuration keys, `GET /api/modes`, `PUT /api/mode`, the `{type: "mode"}`
   WebSocket message, or its `model.before_request` hook. `--mode` is removed
   from `rah web`, `rah-desktop` and `scripts/start.ps1`. The Web UI has no
   mode picker.
2. **An old configuration is refused, not ignored.** `mode` or `modes` in the
   `frontend-web` configuration stops the start with the key named, like any
   unknown key (ADR 0044). The `mode` value an earlier version kept in
   `storage` is no longer read.
3. **What a mode switched stays as configured.** Plugins run as `rah.toml`,
   the features and the Plugins panel say. The workspace and every plugin
   view always show. Nothing is added to model requests by the frontend.
4. **API groups stand alone.** Every `tools-http` instance is an API group
   and a choice of the "Internal API" selector, which shows whenever one
   exists (`GET /api/groups`, `PUT /api/group {id, on}`). As before, at most
   one is connected, groups are off until chosen, the connected one is kept
   in `storage` and restored at start, a group added or edited in the Plugins
   panel waits for its choice, and one started by hand replaces the connected
   group. Sub-modes are gone; a group that needs instructions puts them in
   its tool descriptions.

## Consequences

- This supersedes the mode decisions of ADR 0046 (points 2 to 5; the desktop
  shell over the Web UI stays), points 1 to 3 and 5 of ADR 0047, and
  decision 6 of ADR 0062. ADR 0067 no longer has `--mode` or a stored mode.
- A user who relied on "Daily work" to stop the write tools now does so with
  the permissions setting or the Plugins panel; the policy guards tool calls
  exactly as before, since a mode could never change it.
- The kernel, `rah-api` and `rah-protocol` are unchanged.
- A working setup that bundles several switches can return later as its own
  plugin or as a preset of `rah.toml`, where it is kept and visible.
