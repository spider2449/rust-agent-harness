# ADR 0062: Plugin-Contributed UI Views and a Replaceable Layout

Status: Accepted

Date: 2026-10-30

## Context

ADR 0036 makes every component a plugin, and the frontend is one: a user can
unmount `frontend-web` and mount a different frontend. Inside `frontend-web`,
however, the page is fixed. `index.html`, `app.js` and `app.css` are embedded
with `include_str!`, and every panel (sessions, conversation, files, source
control, Plugins) is written into them. No service in `rah-api` lets another
plugin add a view, so a plugin that has something to show (usage totals,
scheduled jobs, an index's state, a review queue) can only do it by changing
`frontend-web`. That is the "patch, don't replace" path AGENTS.md rules out.

DeepSeek Harness, which RAH was conceived as a counterpart of (ADR 0036),
lets plugins contribute UI. RAH should allow the same without weakening what
the Web UI protects: the page holds the frontend token, and through it the
admin API (mount, unmount, configure, the policy).

Three forms were considered:

| Form | What a plugin supplies | Risk | Effort |
|---|---|---|---|
| A. Declarative | Data in a fixed vocabulary of components; `frontend-web` renders it | None beyond the plugin itself: no plugin code runs in the page | Medium |
| B. Sandboxed frame | Its own HTML/JS in a sandboxed `iframe`, talking `postMessage` to a narrow bridge | A second, smaller API to design and keep bounded | High |
| C. Script module | JS loaded into the page | Plugin code holds the token: it can unmount the policy | Low |

Mounting is trust (ADR 0036), but trust in a plugin's Rust or process code
is not the same as handing it the browser session. C would make any UI
plugin as powerful as the user at the keyboard, and a bug in one would be an
XSS in all. B is worth having only once A cannot express a needed view.

## Decision

1. **Form A.** A new service contract `ui-views` in `rah-api` (module
   `rah_api::ui`). Several instances may be mounted; frontends read them all
   with `get_all`. The kernel, `rah-protocol` and the plugin dependency rules
   are unchanged.

2. **The contract** (sketch; names settle in the implementation):

   ```rust
   pub const UI_VIEWS: ServiceKey<dyn UiViews> = ServiceKey::new("ui-views");

   #[async_trait]
   pub trait UiViews: Send + Sync {
       /// The views this instance offers. Cheap; called on every page load.
       fn views(&self) -> Vec<ViewSpec>;
       /// The current content of one view.
       async fn render(&self, view: &str, cx: &ViewContext) -> Result<View, UiError>;
       /// The user pressed a button or submitted a form in a view.
       async fn act(&self, view: &str, action: &str, input: Value, cx: &ViewContext)
           -> Result<ActionOutcome, UiError>;
   }
   ```

   - `ViewSpec`: `id`, `title`, `slot` and an optional `order`. Slots are a
     closed enum owned by the contract: `SideTab` (a tab in the right-hand
     panel next to Files and Plugins), `SessionAction` (a button in the
     session head) and `HeaderBadge` (a short status in the header).
   - `ViewContext`: the active session id and workspace root, if any. No
     token, no kernel handle.
   - `View`: a tree of components from a fixed vocabulary: `Text`,
     `Markdown`, `KeyValue`, `Table`, `List`, `Progress`, `Button` (names an
     action) and `Form` (text, number, boolean and choice fields; names an
     action). Bounded: at most 2000 nodes and 256 KiB serialized; a larger
     view is shown as an error.
   - `ActionOutcome`: a message to show and whether to re-render.

3. **Only `frontend-web` renders.** Text is escaped; `Markdown` goes through
   the same renderer as the transcript, with raw HTML, images and
   non-`http(s)` links removed. No component carries HTML, script, style or
   a URL the page loads. A plugin cannot change or remove the built-in
   panels, the approvals bar, the `UNGUARDED` badge or the Plugins panel.

4. **Endpoints.** Behind the existing token and checks:
   `GET /api/ui` (every instance's `ViewSpec`s), `GET
   /api/ui/{instance}/{view}` and `POST
   /api/ui/{instance}/{view}/actions/{action}`. A provider may emit a
   `UiChanged { instance, view }` event; `frontend-web` forwards it on the
   WebSocket (`{type: "ui", instance, view}`) and open windows re-render.

5. **Actions are the user's, not the model's.** Like `file-undo` (ADR 0060),
   `ui-views` is not a tool, so the model cannot reach it. An action is
   trusted plugin code run on a user's click; it gets no path to tool
   providers (the tools key stays private to `rah-api`). An action that
   wants the model to do something starts a turn through the `loop` service
   like any frontend, and its tool calls go through `ToolInvoker` and the
   policy as usual.

6. **Modes choose views.** A mode (ADR 0046) may list `views` (`instance/view`
   ids) to show; without the key every view is shown. Hiding a view is
   presentation only, as `workspace = false` is.

7. **The layout is replaceable; today's layout is the default.** A second
   service, `ui-layout` (one active instance, in the same `rah_api::ui`
   module), returns a `Layout`: the panels of the `left` and the `right`
   column, in order, shown as tabs (the first selected; one left panel
   needs no tab bar; an empty column is not shown), and `right_open`,
   whether the right column starts open. Panels are named by id: the
   built-in `sessions`, `files` (the workspace: tree, source control, open
   files, activity) and `plugins`, and later contributed views
   (`instance/view`).
   - The conversation with its composer is always the center, and the
     header (with the `UNGUARDED` badge and the connection state), the
     approvals and the status bar are not panels. No layout can move or
     hide them.
   - `Layout::standard()` in `rah-api` is the current page, expressed as
     data (`sessions` left; `files`, `plugins` right). `frontend-web` uses
     it when no `ui-layout` is mounted, or when the provider does not answer
     within 2 seconds, so the `default` and `minimal` presets look exactly as
     they do now.
   - `GET /api/layout` returns the layout after `Layout::checked`: unknown,
     empty, overlong and twice-placed ids and panels past 32 are dropped,
     each with a line in `problems`, which the page shows as a notice. A
     panel no column names is not shown, and its header button is hidden.
   - The reply also lists `panels`: every id a layout can place (built-in
     and mounted views) with its title and current column, which the
     Plugins panel shows when a `ui-layout-config` is added or configured.
   - Panel sizes, the open or closed right column and its shown panel stay
     per-user `preferences`, as today; `right_open` applies only until the
     user chooses. The layout is read at page load; a newly mounted
     provider applies at the next load.
   - **The browser and the desktop share it.** `rah-desktop` is a shell over
     `frontend-web` (ADR 0046), so the same layout, views and modes apply to
     both without code in the shell. A mode naming a layout `instance` to
     switch to, as it switches plugins, is a later step.

   The layout is implemented first, as this ADR's first step, with
   `ui-layout-config`, a plugin whose configuration is the layout (its
   check refuses ids that are neither built-in panels nor `instance/view`,
   so a misspelling stops the start). The `ui-views` contract of decisions
   1 to 6 follows as its own task.

8. **Static plugins first.** Process plugins (protocol 3) can provide only
   `tools`, hooks and events (ADR 0039). Providing `ui-views` from a process
   plugin needs a protocol bump and is a later, separate task.

9. **`frontend-cli`** may list views (`/views`) and print one as text; it is
   not required to.

## Consequences

- A plugin gains a view without touching `frontend-web`, and unmounting the
  plugin removes it. The first provider should be a small existing plugin
  with something worth showing (for example a token usage view in
  `telemetry-log`) so the contract is shaped by a real use.
- The component vocabulary limits what a view can look like. A view that
  needs charts or custom interaction waits for a vocabulary addition or for
  form B in a later ADR.
- `SECURITY.md` gains a section: plugin views are data rendered by
  `frontend-web`, carry no code, and cannot hide the guard state.
- Tests: contract types and bounds in `rah-api`; endpoints, escaping,
  oversize views, unknown instances and actions, and `UiChanged` forwarding
  in `frontend-web`; a mode that lists `views`. No live model is needed.
- `app.js` has to build the page from a `Layout` instead of the fixed
  `index.html`. This is the largest part of the work and comes first, with
  `Layout::standard()` only: the page must look and behave the same before
  any layout plugin exists.
- Layout tests: `Layout::standard()` places every built-in panel once and
  passes the check; unknown, duplicate and overlong ids are dropped with a
  problem; `/api/layout` follows a provider being mounted and unmounted;
  in a real browser the standard page is unchanged and a provider's layout
  moves the panels.
- `rah-api` grows by one module; the architecture tests are unchanged.

## Implementation status

Done, in five steps:

1. The layout (decision 7): `ui-layout`, `Layout::standard()`,
   `GET /api/layout` and the page built from it.
2. `ui-layout-config`, the layout as plugin configuration.
3. `ui-views` (decisions 1 to 5), as follows:
   - `ViewSpec` is `id` and `title`. Every view is a panel with the id
     `<instance>/<view>`, which a layout places like a built-in panel; a
     view no column names joins the right column, and a placed view of a
     provider that is not mounted is left out without a problem. To hide a
     view, unmount its plugin.
   - `ViewContext` carries the session open in the window.
   - Nodes: `heading`, `text`, `markdown`, `key_value`, `table`, `list`,
     `button` and `form` (text, number, boolean and choice fields). `View::check`
     enforces 2000 nodes and 256 KiB; a provider has 10 seconds to render or
     act.
   - The page renders a view when its tab is shown, on its reload button,
     after an action that asks for it, and when a turn of the open session
     ends.
   - The first provider is `telemetry-log`: a `usage` view of the turns,
     model requests, tokens and tool calls this process recorded, with a
     Reset button.
4. `UiChanged { instance, view }` (decision 4), an event in `rah_api::ui`.
   `frontend-web` forwards it to every window as `{type: "ui", instance,
   view}`, dropping one whose view is not a valid id; a window renders the
   view again 150 ms after the last change if it is shown. `telemetry-log`
   emits it once its queued records are written, so the usage view follows
   turns of every session.
5. Modes choose views (decision 6): a mode's `views` lists the view ids it
   shows; `[]` hides every view, and without the key every view shows. A
   sub-mode inherits its parent's list. The configuration check refuses an
   entry that is not `instance/view`. A hidden view's tab is hidden, and a
   column showing it moves to another panel; the plugin keeps running.

Not done yet: the `SessionAction` and `HeaderBadge` slots and `order`, a
`Progress` node and views in `frontend-cli` (decision 9). Views from process
plugins (decision 8) are protocol 4, ADR 0063.
