# ADR 0063: Views from Process Plugins (Protocol 4)

Status: Accepted

Date: 2026-10-30

## Context

ADR 0062 lets plugins add views to the Web UI through the `ui-views`
service, and lets a provider announce a change with `UiChanged`. Only
static Rust plugins can do so: a process plugin (protocol 3, ADR 0039) may
provide `tools` only, and the host ignores notifications from the plugin.
A view of an external system (a build queue, a ticket list, a deploy
switch) is exactly what a plugin in another language is often written for.

A v3 host refuses a manifest whose `provides` names anything but `tools`,
so a plugin that needs views fails closed on an older `rah`, as ADR 0039
requires of new manifest fields.

## Decision

1. **Protocol 4.** The host speaks protocol 4 (`initialize.protocol` and
   `RAH_PLUGIN_PROTOCOL` are `4`) and still accepts manifests answering
   `2` (tools only) or `3` (adds hooks and events). Views need `4`.

2. **Manifest.** `provides` may contain `ui-views` exactly when `views`
   is non-empty: `[{"id": "<view>", "title": "<tab label>"}]`, at most 32,
   ids unique and valid view ids (lowercase letters, digits, `-`, `_`; at
   most 64 bytes). The views take part in the check that a restarted
   plugin's manifest is unchanged. The instance is the plugin's id, so a
   view's panel id is `<plugin id>/<view>`.

3. **Rendering and actions.** The host sends requests, each bounded by
   `view_timeout_ms` (default 5000) and never retried:
   - `views/render {view, session}`: the result is a view in the
     `rah_api::ui::View` JSON form (`{"nodes": [...]}`). The host parses it
     strictly (unknown node types or fields make it malformed) and the
     frontend still applies the size bounds.
   - `views/act {view, action, input, session}`: the result is
     `{"message"?, "rerender"?}`.
   `session` is the session open in the window, or `null`. An error
   response whose `data.kind` is `unknown_view`, `unknown_action` or
   `invalid_input` maps to that `UiError`; any other error, a timeout, a
   lost connection or a malformed result is a failed view or action, shown
   to the user. An action that timed out may still have happened; the host
   does not repeat it.

4. **Changes.** The plugin sends the notification `ui/changed {view}`. For
   a view in its manifest the host emits `UiChanged {instance: <plugin id>,
   view}`; other views are ignored. These are the first notifications the
   host accepts from a plugin; any other plugin notification is still
   ignored.

5. **Unchanged.** Views reach the model through nothing: they are not
   tools. The kernel and the `rah-api` contracts do not change. A v4 plugin
   that provides `ui-views` mounted on a v3 host is refused ("cannot
   provide `ui-views`").

## Consequences

- A plugin in any language can add panels, with buttons and forms, and
  keep them current.
- Each render and action is a round trip over the pipe; a view must fit in
  `max_message_bytes` as well as in the frontend's 256 KiB bound.
- `rah-plugin-host-process` gains `Peer::connect_with_notifications`, and
  `docs/PROCESS_PLUGINS.md` describes protocol 4.
