# ADR 0046: A Desktop Shell over the Web UI, and Modes Shared by Both

Status: Accepted

Date: 2026-10-16

## Context

The legacy Desktop (Tauri) was removed in P5 and the Web UI (`frontend-web`)
replaced it. A desktop version is wanted again, with modes such as daily work
and coding. The requirement is that every behavior exists once: a change to
how sessions, turns, approvals, settings or modes work must reach the browser
and the desktop without being made twice.

The legacy Desktop showed the cost of the opposite: its own backend commands
(47 Tauri invoke handlers), its own permission set and its own tests, drifting
from the host they wrapped.

## Decision

1. **The desktop is a shell, not a frontend.** `crates/rah-desktop` builds the
   same distribution as `rah --web` (through `rah_cli::select_distribution`),
   with `frontend-web` on a free loopback port, boots it in-process, and opens
   one native window (tao) whose WebView (wry: WebView2 on Windows, WebKit on
   macOS and Linux) loads the announced URL with its token. It adds no API,
   no IPC commands and no UI code. Closing the window shuts the kernel down; a
   plugin's `ExitRequested` closes the window. A start failure is shown in the
   window, since a release build has no console on Windows.
2. **Modes live in `frontend-web`.** A mode (`id`, `label`, `description`,
   `instructions`, `plugins`, `workspace`) is defined in one place: the
   plugin's `modes` configuration, else the built-in `daily` and `coding`.
   `GET /api/modes` lists them and `PUT /api/mode {mode}` switches; the
   browser and the desktop use the same endpoints and the same picker.
3. **A mode switches plugins through `admin`**, as the Plugins panel and the
   web search switch do: `true` starts a stopped instance of the kind or mounts
   one, `false` stops it (keeping its configuration and workspace root).
   `tools-web-search` goes through the web search switch, so the policy asks
   before `web.*` first. A mode cannot list `policy-*` or `frontend-*` kinds:
   a mode never changes what guards tool calls, and the policy still decides
   every call of a tool a mode mounts.
4. **One mode is active per `rah`, not per window.** Plugins are shared state,
   so a per-window mode could not mean different tools. A switch is pushed to
   every connected window (`{type: "mode", active, problems}` on the
   WebSocket) and stored in `storage`; the next start enters it again after
   the stored settings. The `mode` configuration key (`--mode` on `rah` and
   `rah-desktop`) picks the mode at start instead.
5. **Instructions are a hook.** `frontend-web` adds the active mode's
   instructions as a system item after the leading ones on
   `model.before_request`. The system prompt in Settings is untouched.
6. **`workspace = false` is presentation only.** It hides the files view, the
   Git state and the folder in the UI; the tools a mode keeps still work.

## Consequences

- `rah-desktop` depends on `rah-cli` (the distribution and catalog), `wry` and
  `tao`. Building it on Linux needs the WebKitGTK and GTK development packages,
  which CI installs. The plugin dependency rules are unchanged: the shell is a
  distribution binary, not a plugin.
- Anything the desktop should do differently from a browser must be a mode or
  a Web UI behavior, not code in the shell.
- Switching modes restarts the switched plugins, like the Plugins panel; a
  turn running at that moment may lose a tool it was about to call.
- A plugin a mode cannot switch is reported in `problems`; the mode is still
  entered, so the windows agree on which mode is active.
